use base64::encode;
use failure::{format_err, Error, Fail};
use rocket::config::{Config, Environment};
use rocket::http::{Method, RawStr, Status};
use rocket::request::{self, FromRequest, Request};
use rocket::response::status::Custom;
use rocket::response::Redirect;
use rocket::{catch, catchers, delete, get, post, put, routes, Outcome, State};
use rocket_contrib::json::Json;
use secstr::SecStr;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs::read_to_string;
use std::path::PathBuf;

use crate::config::{GitTrigger, Trigger};
use crate::model::queues::{JobSummary, Queues};
use crate::model::repositories::{Repositories, Repository};
use crate::model::users::{UpdateUserPassword, User, Users};
use crate::queue::{ArbitraryData, QueueItem};
use crate::AppState;

#[allow(unused_imports)]
use log::{debug, error, info, warn};

mod auth;
mod git;
mod github;
pub mod response;
mod static_assets;

use auth::{authenticate_user, AuthenticationPayload, UserPayload};
use git::GitReference;
use github::GitHubPayload;
use response::{AppConfigResponse, ErrorResponse, RepositoryResponse, Response, UserResponse};
use static_assets::{AssetType, Assets};

pub struct SecretKey;

// TODO do error handling better
#[derive(Fail, Debug, Clone)]
pub enum SecretKeyError {
	#[fail(display = "Signature was not found")]
	Missing,
	#[fail(display = "Signature is invalid")]
	Invalid,
	#[fail(display = "Invalid payload")]
	BadData,
	#[fail(display = "Unhandled error")]
	Unknown,
}

fn secret_key_is_valid(secret: &str, repository: &Repository) -> bool {
	let secret = SecStr::from(secret);
	let repository_secret = SecStr::from(repository.secret.clone());
	secret == repository_secret
}

const NOTIFY_ROUTE_SLUG_INDEX: usize = 1;

impl<'a, 'r> FromRequest<'a, 'r> for SecretKey {
	type Error = SecretKeyError;

	fn from_request(request: &'a Request<'r>) -> request::Outcome<Self, SecretKeyError> {
		let repository_slug = request
			.get_param(NOTIFY_ROUTE_SLUG_INDEX)
			.and_then(|r: Result<&RawStr, _>| r.ok())
			.expect("Invalid route")
			.as_str();

		let state = request.guard::<State<AppState>>().unwrap();
		let repository =
			Repositories::new(state.connection_manager.clone()).find_by_slug(repository_slug);

		if repository.is_none() {
			return Outcome::Failure((Status::NotFound, SecretKeyError::Invalid));
		}

		let repository = repository.unwrap();

		let secret_key = request.headers().get("x-secret-key").next();
		match secret_key {
			Some(secret_key) => {
				if secret_key_is_valid(&secret_key, &repository) {
					Outcome::Success(SecretKey)
				} else {
					Outcome::Failure((Status::BadRequest, SecretKeyError::Invalid))
				}
			}
			_ => {
				let secret_key: Option<&RawStr> =
					request.get_query_value("key").and_then(|r| r.ok());
				match secret_key {
					Some(secret_key) => {
						let secret_key = secret_key.as_str();
						if secret_key_is_valid(&secret_key, &repository) {
							Outcome::Success(SecretKey)
						} else {
							Outcome::Failure((Status::BadRequest, SecretKeyError::Invalid))
						}
					}
					_ => Outcome::Failure((Status::BadRequest, SecretKeyError::Missing)),
				}
			}
		}
	}
}

fn notify_new_job(
	repository: &str,
	values: ArbitraryData,
	state: &AppState,
) -> Result<Response<QueueItem>, String> {
	match state.queue_manager.push(repository, values) {
		Ok(item) => Ok(Response { response: item }),
		Err(error) => Err(format!("{}", error)),
	}
}

fn notify_job(
	repository: &RawStr,
	values: ArbitraryData,
	state: &AppState,
) -> Result<Json<Response<QueueItem>>, Custom<Json<ErrorResponse>>> {
	match notify_new_job(repository.as_str(), values, state) {
		Ok(job) => Ok(Json(job)),
		Err(error) => Err(Custom(
			Status::InternalServerError,
			Json(ErrorResponse::new(error)),
		)),
	}
}

#[get("/notify/<repository>")]
pub fn notify(
	repository: &RawStr,
	_secret_key: SecretKey,
	state: State<AppState>,
) -> Result<Json<Response<QueueItem>>, Custom<Json<ErrorResponse>>> {
	notify_job(
		repository,
		ArbitraryData::new(HashMap::new()),
		state.inner(),
	)
}

#[post("/notify/<repository>", format = "json", data = "<data>")]
pub fn notify_with_data(
	repository: &RawStr,
	data: Json<ArbitraryData>,
	_secret_key: SecretKey,
	state: State<AppState>,
) -> Result<Json<Response<QueueItem>>, Custom<Json<ErrorResponse>>> {
	notify_job(repository, data.into_inner(), state.inner())
}

#[derive(Serialize, Clone, Debug)]
pub enum JobOrSkipped {
	#[serde(rename = "skipped")]
	Skipped(String),
	#[serde(rename = "job")]
	Job(Response<QueueItem>),
}

#[post("/notify/<repository>/github", format = "json", data = "<payload>")]
pub fn notify_github(
	repository: &RawStr,
	payload: GitHubPayload,
	state: State<AppState>,
) -> Result<Json<JobOrSkipped>, Custom<Json<ErrorResponse>>> {
	let repository_name = repository.as_str();

	let repository =
		Repositories::new(state.connection_manager.clone()).find_by_slug(repository_name);
	let repository = match repository {
		Some(repository) => repository,
		None => {
			return Err(Custom(
				Status::NotFound,
				Json(ErrorResponse::new(
					format!("Repository `{}` not found", repository_name).into(),
				)),
			))
		}
	};

	let mut should_skip = true;
	let triggers = repository.triggers.clone();
	for trigger in triggers.into_iter() {
		match trigger {
			Trigger::Any => {
				debug!("Matched any trigger for repository {}", repository_name);
				should_skip = false;
				break;
			}
			Trigger::Git(GitTrigger::Any) => {
				debug!("Matched any git trigger for repository {}", repository_name);
				should_skip = false;
				break;
			}
			Trigger::Git(GitTrigger::Tag) => {
				debug!("Matched tag trigger");
				if let GitReference::Tag(_) = &payload.reference {
					debug!("Matched tag trigger for repository {}", repository_name);
					should_skip = false;
				}
			}
			Trigger::Git(GitTrigger::Head(refs)) => {
				for trigger_ref in refs.iter() {
					if let GitReference::Head(payload_ref) = &payload.reference {
						if *trigger_ref == *payload_ref {
							debug!(
								"Matched head trigger {} for repository {}",
								&trigger_ref, repository_name
							);
							should_skip = false;
						}
					}
				}
			}
		}
	}

	if should_skip {
		debug!("Skipping job for repository {}", repository_name);
		Ok(Json(JobOrSkipped::Skipped(
			"Trigger rules not matched. No job queued".into(),
		)))
	} else {
		debug!("Notifying new job for repository {}", repository_name);
		match notify_new_job(repository_name, ArbitraryData::from(payload), state.inner()) {
			Ok(response) => Ok(Json(JobOrSkipped::Job(response))),
			Err(error) => Err(Custom(
				Status::InternalServerError,
				Json(ErrorResponse::new(
					format!("Unable to add job to the queue. {}", error).into(),
				)),
			)),
		}
	}
}

#[get("/repositories")]
pub fn repositories(
	_auth: AuthenticationPayload,
	state: State<AppState>,
) -> Result<Json<Vec<Response<RepositoryResponse>>>, Custom<Json<ErrorResponse>>> {
	Ok(Json(
		Repositories::new(state.connection_manager.clone())
			.all()
			.into_iter()
			.map(|r| {
				let repository = RepositoryResponse::from(r);
				Response {
					response: repository,
				}
			})
			.collect(),
	))
}

#[get("/config")]
pub fn get_config(
	_auth: AuthenticationPayload,
	state: State<AppState>,
) -> Result<Json<AppConfigResponse>, ()> {
	Ok(Json(AppConfigResponse::from(state.config.clone())))
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserCredentials {
	pub username: String,
	pub password: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoginResponse {
	#[serde(flatten)]
	pub payload: UserPayload,
	pub token: String,
}

#[post("/login", format = "json", data = "<data>")]
pub fn login(
	data: Json<UserCredentials>,
	state: State<AppState>,
) -> Result<Json<LoginResponse>, Custom<Json<ErrorResponse>>> {
	let data = data.into_inner();
	let payload = authenticate_user(
		state.config.clone(),
		state.connection_manager.clone(),
		&data.username,
		&data.password,
	);
	match payload {
		Ok(payload) => {
			let response = LoginResponse {
				payload: payload.clone(),
				token: payload.into_token(&state.config),
			};
			Ok(Json(response))
		}
		Err(_) => Err(Custom(
			Status::Unauthorized,
			Json(ErrorResponse::new("Username or password incorrect".into())),
		)),
	}
}

#[get("/users")]
pub fn users(
	_auth: AuthenticationPayload,
	state: State<AppState>,
) -> Result<Json<Vec<UserResponse>>, ()> {
	Ok(Json(
		Users::new(state.connection_manager.clone())
			.all()
			.into_iter()
			.map(|r| UserResponse::from(r))
			.collect(),
	))
}

#[get("/users/<id>")]
pub fn get_user(
	id: &RawStr,
	_auth: AuthenticationPayload,
	state: State<AppState>,
) -> Result<Json<UserResponse>, Custom<Json<ErrorResponse>>> {
	let id = id.as_str();
	let result = Users::new(state.connection_manager.clone()).find_by_id(&id);
	match result {
		Some(user) => Ok(Json(UserResponse::from(user))),
		None => {
			warn!("Could not find user with ID {}", &id);
			return Err(Custom(
				Status::NotFound,
				Json(ErrorResponse::new("User not found".into())),
			));
		}
	}
}

#[delete("/users/<id>")]
pub fn delete_user(
	id: &RawStr,
	_auth: AuthenticationPayload,
	state: State<AppState>,
) -> Result<(), Custom<Json<ErrorResponse>>> {
	let result = Users::new(state.connection_manager.clone()).delete_by_id(id.as_str());
	match result {
		Ok(_) => Ok(()),
		Err(error) => {
			error!("Error deleting user: {}", error);

			Err(Custom(
				Status::BadRequest,
				Json(ErrorResponse::new(format!("Could not delete user").into())),
			))
		}
	}
}

#[post("/users", format = "json", data = "<data>")]
pub fn add_user(
	data: Json<User>,
	_auth: AuthenticationPayload,
	state: State<AppState>,
) -> Result<Json<UserResponse>, Custom<Json<ErrorResponse>>> {
	let data = data.into_inner();
	let user = Users::new(state.connection_manager.clone()).create(data);
	match user {
		Ok(user) => Ok(Json(UserResponse::from(user))),
		Err(error) => {
			error!("Error adding user: {}", error);

			Err(Custom(
				Status::BadRequest,
				Json(ErrorResponse::new("Could not create new user.".into())),
			))
		}
	}
}

#[put("/users", format = "json", data = "<data>")]
pub fn update_user(
	data: Json<User>,
	_auth: AuthenticationPayload,
	state: State<AppState>,
) -> Result<Json<UserResponse>, Custom<Json<ErrorResponse>>> {
	let data = data.into_inner();
	let record = Users::new(state.connection_manager.clone()).save(data);
	match record {
		Ok(record) => Ok(Json(UserResponse::from(record))),
		Err(error) => {
			error!("Error saving user, {}", error);

			Err(Custom(
				Status::BadRequest,
				Json(ErrorResponse::new("Could not update user".into())),
			))
		}
	}
}

#[put("/users/password", format = "json", data = "<data>")]
pub fn set_password(
	data: Json<UpdateUserPassword>,
	auth: AuthenticationPayload,
	state: State<AppState>,
) -> Result<(), Custom<Json<ErrorResponse>>> {
	let user_payload: Option<UserPayload> = auth.into();

	match user_payload {
		Some(user_payload) => {
			let data = data.into_inner();
			let result = Users::new(state.connection_manager.clone())
				.set_password(&user_payload.username, data);
			match result {
				Ok(()) => Ok(()),
				Err(error) => {
					error!("Error setting password, {}", error);

					Err(Custom(
						Status::BadRequest,
						Json(ErrorResponse::new(format!("Could not set password").into())),
					))
				}
			}
		}
		None => {
			warn!("Cannot set password when auth is disabled");

			Err(Custom(
				Status::BadRequest,
				Json(ErrorResponse::new(
					format!("Cannot set password when auth is disabled").into(),
				)),
			))
		}
	}
}

#[get("/repositories/<repository>")]
pub fn repository(
	repository: &RawStr,
	_auth: AuthenticationPayload,
	state: State<AppState>,
) -> Result<Json<Response<RepositoryResponse>>, Custom<Json<ErrorResponse>>> {
	let record =
		Repositories::new(state.connection_manager.clone()).find_by_slug(repository.as_str());
	match record {
		Some(record) => {
			let repository = RepositoryResponse::from(record);
			Ok(Json(Response {
				response: repository,
			}))
		}
		None => Err(Custom(
			Status::NotFound,
			Json(ErrorResponse::new(
				format!("Repository `{}` not found", repository).into(),
			)),
		)),
	}
}

#[post("/repositories", format = "json", data = "<data>")]
pub fn add_repository(
	data: Json<Repository>,
	_auth: AuthenticationPayload,
	state: State<AppState>,
) -> Result<Json<Response<RepositoryResponse>>, Custom<Json<ErrorResponse>>> {
	let data = data.into_inner();
	let record = Repositories::new(state.connection_manager.clone()).create(data);
	match record {
		Ok(record) => {
			let repository = RepositoryResponse::from(record);
			Ok(Json(Response {
				response: repository,
			}))
		}
		Err(error) => Err(Custom(
			Status::BadRequest,
			Json(ErrorResponse::new(
				format!("Could not create new repository. {}", error).into(),
			)),
		)),
	}
}

#[put("/repositories", format = "json", data = "<data>")]
pub fn update_repository(
	data: Json<Repository>,
	_auth: AuthenticationPayload,
	state: State<AppState>,
) -> Result<Json<Response<RepositoryResponse>>, Custom<Json<ErrorResponse>>> {
	let data = data.into_inner();
	let record = Repositories::new(state.connection_manager.clone()).save(data);
	match record {
		Ok(record) => {
			let repository = RepositoryResponse::from(record);
			Ok(Json(Response {
				response: repository,
			}))
		}
		Err(error) => {
			error!("Error saving repository: {}", error);

			Err(Custom(
				Status::BadRequest,
				Json(ErrorResponse::new(
					format!("Could not update repository").into(),
				)),
			))
		}
	}
}

#[delete("/repositories/<id>")]
pub fn delete_repository(
	id: &RawStr,
	_auth: AuthenticationPayload,
	state: State<AppState>,
) -> Result<(), Custom<Json<ErrorResponse>>> {
	let repository_id = id.as_str();
	let result = Repositories::new(state.connection_manager.clone()).delete_by_id(id.as_str());
	match result {
		Ok(_) => {
			state.queue_manager.notify_deleted(&repository_id);
			Ok(())
		}
		Err(error) => {
			error!("Error deleting repository: {}", error);

			Err(Custom(
				Status::BadRequest,
				Json(ErrorResponse::new(
					format!("Could not delete repository").into(),
				)),
			))
		}
	}
}

#[get("/jobs")]
pub fn all_jobs(
	_auth: AuthenticationPayload,
	state: State<AppState>,
) -> Result<Json<Vec<JobSummary>>, Custom<Json<ErrorResponse>>> {
	let queues_model = Queues::new(state.connection_manager.clone());
	match queues_model.all() {
		Ok(jobs) => Ok(Json(jobs)),
		Err(error) => {
			error!("Unable to fetch jobs. {}", error);
			Err(Custom(
				Status::NotFound,
				Json(ErrorResponse::new("Unable to fetch jobs.".into())),
			))
		}
	}
}

#[get("/repositories/<repository>/jobs")]
pub fn jobs(
	repository: &RawStr,
	_auth: AuthenticationPayload,
	state: State<AppState>,
) -> Result<Json<Vec<Response<QueueItem>>>, Custom<Json<ErrorResponse>>> {
	let repository = repository.as_str();
	let record = Repositories::new(state.connection_manager.clone()).find_by_slug(repository);
	let repository = match record {
		// We just need the repository slug
		Some(repository) => repository,
		None => {
			return Err(Custom(
				Status::NotFound,
				Json(ErrorResponse::new(
					format!("Repository `{}` not found", repository).into(),
				)),
			))
		}
	};

	let queues_model = Queues::new(state.connection_manager.clone());
	match queues_model.all_for_repository(&repository.id) {
		Ok(jobs) => Ok(Json(
			jobs.into_iter()
				.map(|job| Response { response: job })
				.collect(),
		)),
		Err(error) => Err(Custom(
			Status::InternalServerError,
			Json(ErrorResponse::new(
				format!(
					"Unable to fetch jobs for repository {}. {}",
					repository.slug, error
				)
				.into(),
			)),
		)),
	}
}

#[get("/repositories/<repository>/jobs/<id>/output")]
pub fn log_output(
	repository: &RawStr,
	id: &RawStr,
	_auth: AuthenticationPayload,
	state: State<AppState>,
) -> Result<String, Custom<String>> {
	let repository = repository.as_str();
	let record = Repositories::new(state.connection_manager.clone()).find_by_slug(repository);
	let repository = match record {
		// We just need the repository slug
		Some(repository) => repository,
		None => {
			return Err(Custom(
				Status::NotFound,
				format!("Repository `{}` does not exist", repository).into(),
			));
		}
	};

	let id = id.as_str();

	let queues_model = Queues::new(state.connection_manager.clone());
	match queues_model.job(&repository.id, &id) {
		Ok(job) => {
			let log_output = read_to_string(format!(
				"{}/jobs/{}/output.log",
				&state.config.data_dir, &job.id
			));
			match log_output {
				Ok(log_output) => Ok(log_output),
				Err(_) => Err(Custom(
					Status::InternalServerError,
					format!("Unable to read output file for job `{}`", &id).into(),
				)),
			}
		}
		Err(_) => Err(Custom(
			Status::NotFound,
			format!(
				"Couldn't find job `{}` for repository `{}`",
				&id, &repository.slug
			)
			.into(),
		)),
	}
}

#[get("/repositories/<repository>/jobs/<id>")]
pub fn job(
	repository: &RawStr,
	id: &RawStr,
	_auth: AuthenticationPayload,
	state: State<AppState>,
) -> Result<Json<Response<QueueItem>>, Custom<Json<ErrorResponse>>> {
	let repository = repository.as_str();
	let record = Repositories::new(state.connection_manager.clone()).find_by_slug(repository);
	let repository = match record {
		// We just need the repository slug
		Some(repository) => repository,
		None => {
			return Err(Custom(
				Status::NotFound,
				Json(ErrorResponse::new(
					format!("Repository `{}` does not exist", repository).into(),
				)),
			));
		}
	};

	let id = id.as_str();

	let queues_model = Queues::new(state.connection_manager.clone());
	match queues_model.job(&repository.id, &id) {
		Ok(job) => Ok(Json(Response { response: job })),
		Err(_) => Err(Custom(
			Status::NotFound,
			Json(ErrorResponse::new(
				format!(
					"Couldn't find job `{}` for repository `{}`",
					&id, &repository.slug
				)
				.into(),
			)),
		)),
	}
}

#[get("/static/<file..>")]
pub fn get_static_asset(file: PathBuf) -> Assets {
	Assets {
		file_path: file,
		asset_type: AssetType::StaticAssets,
	}
}

#[get("/swagger/<file..>")]
pub fn get_swagger_asset(file: PathBuf) -> Assets {
	Assets {
		file_path: file,
		asset_type: AssetType::ApiDefinitionUi,
	}
}

#[get("/swagger")]
pub fn swagger() -> Redirect {
	Redirect::to("/swagger/index.html")
}

// TODO Disable this if in debug mode too
#[get("/ui/<file..>")]
pub fn get_ui_asset(file: PathBuf) -> Assets {
	Assets {
		file_path: file,
		asset_type: AssetType::UI,
	}
}

#[get("/ui")]
pub fn ui() -> Redirect {
	if cfg!(debug_assertions) {
		panic!("UI not available in debug mode")
	} else {
		Redirect::to("/ui/index.html")
	}
}

#[catch(404)]
pub fn not_found_handler() -> Json<ErrorResponse> {
	Json(ErrorResponse::new("Not found".into()))
}

use rocket_cors::{
	AllowedHeaders,
	AllowedOrigins, // 2.
	Cors,
	CorsOptions, // 3.
};

pub fn create_cors_options() -> Cors {
	CorsOptions {
		allowed_origins: AllowedOrigins::all(),
		allowed_methods: vec![Method::Get, Method::Put, Method::Post, Method::Delete]
			.into_iter()
			.map(From::from)
			.collect(),
		allowed_headers: AllowedHeaders::all(),
		allow_credentials: true,
		..Default::default()
	}
	.to_cors()
	.expect("Unable to build CORS Options")
}

pub fn start_server(app_state: AppState) -> Result<(), Error> {
	let http_config = Config::build(Environment::Production)
		// This should never use cookies though?
		.secret_key(encode(&nanoid::generate(32)))
		.address(&app_state.config.network_host)
		.port(app_state.config.port)
		.workers(1)
		.keep_alive(0)
		.finalize();

	match http_config {
		Ok(config) => {
			let routes = routes![
				get_config,
				notify,
				notify_with_data,
				notify_github,
				repositories,
				repository,
				add_repository,
				update_repository,
				delete_repository,
				all_jobs,
				jobs,
				job,
				log_output,
				login,
				users,
				get_user,
				delete_user,
				add_user,
				update_user,
				set_password,
				get_static_asset,
				// TODO ??? remove swagger UI
				get_swagger_asset,
				swagger,
				get_ui_asset,
				ui,
			];

			// Rocket log formatting makes output messy
			env::set_var("ROCKET_CLI_COLORS", "off");

			let server = rocket::custom(config)
				.attach(create_cors_options())
				.manage(app_state)
				.register(catchers![not_found_handler])
				.mount("/", routes);

			server.launch();
		}
		Err(error) => return Err(format_err!("Invalid HTTP configuration: {}", error)),
	};

	Ok(())
}
