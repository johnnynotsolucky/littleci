use base64::encode;
use failure::{format_err, Error, Fail};
use rocket::config::{Config, Environment};
use rocket::http::{ContentType, Method, RawStr, Status};
use rocket::request::{self, FromRequest, Request};
use rocket::response::status::Custom;
use rocket::response::{Redirect, Responder};
use rocket::{catch, catchers, get, post, routes, Outcome, State};
use rocket_contrib::json::Json;
use secstr::SecStr;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs::read_to_string;
use std::io::Cursor;
use std::path::PathBuf;

use crate::config::{GitTrigger, PersistedConfig, Trigger};
use crate::model::repositories::{Repositories, Repository};
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
use response::{
	meta_for_queue_item, meta_for_repository, AppConfigResponse, ErrorResponse, RepositoryResponse,
	Response, RouteMap, Routes,
};
use static_assets::{ApiDefinitionUi, StaticAssets};

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
	println!(
		"{} == {}",
		std::str::from_utf8(secret.clone().unsecure()).unwrap(),
		std::str::from_utf8(repository_secret.clone().unsecure()).unwrap(),
	);
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
		let repository = Repositories::new(state.config.clone()).find_by_slug(repository_slug);

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
	routes: &RouteMap,
) -> Result<Response<QueueItem>, String> {
	match state.queue_manager.push(repository, values) {
		Ok(item) => Ok(Response {
			meta: meta_for_queue_item(&state.config, &routes, &item),
			response: item,
		}),
		Err(error) => Err(format!("{}", error)),
	}
}

fn notify_job(
	repository: &RawStr,
	values: ArbitraryData,
	state: &AppState,
	routes: &RouteMap,
) -> Result<Json<Response<QueueItem>>, String> {
	match notify_new_job(repository.as_str(), values, state, routes) {
		Ok(job) => Ok(Json(job)),
		Err(error) => Err(error),
	}
}

#[get("/notify/<repository>")]
pub fn notify(
	repository: &RawStr,
	_secret_key: SecretKey,
	state: State<AppState>,
	routes: State<RouteMap>,
) -> Result<Json<Response<QueueItem>>, String> {
	notify_job(
		repository,
		ArbitraryData::new(HashMap::new()),
		state.inner(),
		routes.inner(),
	)
}

#[post("/notify/<repository>", format = "json", data = "<data>")]
pub fn notify_with_data(
	repository: &RawStr,
	data: Json<ArbitraryData>,
	_secret_key: SecretKey,
	state: State<AppState>,
	routes: State<RouteMap>,
) -> Result<Json<Response<QueueItem>>, String> {
	notify_job(repository, data.into_inner(), state.inner(), routes.inner())
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
	routes: State<RouteMap>,
) -> Result<Json<JobOrSkipped>, Custom<Json<ErrorResponse>>> {
	let repository_name = repository.as_str();

	let repository = Repositories::new(state.config.clone()).find_by_slug(repository_name);
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
				debug!("Trigger tag");
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
		match notify_new_job(
			repository_name,
			ArbitraryData::from(payload),
			state.inner(),
			routes.inner(),
		) {
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
	routes: State<RouteMap>,
) -> Result<Json<Vec<Response<RepositoryResponse>>>, String> {
	Ok(Json(
		Repositories::new(state.config.clone())
			.all()
			.into_iter()
			.map(|r| {
				let repository = RepositoryResponse::from(r);
				Response {
					meta: meta_for_repository(&state.config, &routes, &repository),
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
) -> Result<Json<AppConfigResponse>, String> {
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
	let payload = authenticate_user(state.config.clone(), &data.username, &data.password);
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

#[get("/repositories/<repository>")]
pub fn repository(
	repository: &RawStr,
	_auth: AuthenticationPayload,
	state: State<AppState>,
	routes: State<RouteMap>,
) -> Result<Json<Response<RepositoryResponse>>, String> {
	let record = Repositories::new(state.config.clone()).find_by_slug(repository.as_str());
	match record {
		Some(record) => {
			let repository = RepositoryResponse::from(record);
			Ok(Json(Response {
				meta: meta_for_repository(&state.config, &routes, &repository),
				response: repository,
			}))
		}
		None => Err(format!("Repository `{}` does not exist", repository)),
	}
}

#[get("/repositories/<repository>/jobs")]
pub fn jobs(
	repository: &RawStr,
	_auth: AuthenticationPayload,
	state: State<AppState>,
	routes: State<RouteMap>,
) -> Result<Json<Vec<Response<QueueItem>>>, String> {
	let repository = repository.as_str();
	let record = Repositories::new(state.config.clone()).find_by_slug(repository);
	let repository = match record {
		// We just need the repository slug
		Some(_) => repository,
		None => return Err(format!("Repository `{}` does not exist", repository)),
	};

	match state.queue_manager.all(&repository) {
		Ok(jobs) => Ok(Json(
			jobs.into_iter()
				.map(|job| Response {
					meta: meta_for_queue_item(&state.config, &routes, &job),
					response: job,
				})
				.collect(),
		)),
		Err(error) => Err(format!(
			"Unable to fetch jobs for repository {}. {}",
			repository, error
		)),
	}
}

#[get("/repositories/<repository>/jobs/<id>/output")]
pub fn log_output(
	repository: &RawStr,
	id: &RawStr,
	_auth: AuthenticationPayload,
	state: State<AppState>,
) -> Result<String, String> {
	let repository = repository.as_str();
	let record = Repositories::new(state.config.clone()).find_by_slug(repository);
	let repository = match record {
		// We just need the repository slug
		Some(_) => repository,
		None => return Err(format!("Repository `{}` does not exist", repository)),
	};

	let id = id.as_str();

	match state.queue_manager.job(&repository, &id) {
		Ok(job) => {
			let log_output = read_to_string(format!(
				"{}/jobs/{}/output.log",
				&state.config.data_dir, &job.id
			));
			match log_output {
				Ok(log_output) => Ok(log_output),
				Err(error) => Err(format!(
					"Unable to read output file for job {}. {}",
					&id, error
				)),
			}
		}
		Err(error) => Err(format!(
			"Unable to fetch jobs for repository {}. {}",
			repository, error
		)),
	}
}

#[get("/repositories/<repository>/jobs/<id>")]
pub fn job(
	repository: &RawStr,
	id: &RawStr,
	_auth: AuthenticationPayload,
	state: State<AppState>,
	routes: State<RouteMap>,
) -> Result<Json<Response<QueueItem>>, String> {
	let repository = repository.as_str();
	let record = Repositories::new(state.config.clone()).find_by_slug(repository);
	let repository = match record {
		// We just need the repository slug
		Some(_) => repository,
		None => return Err(format!("Repository `{}` does not exist", repository)),
	};

	let id = id.as_str();

	match state.queue_manager.job(&repository, &id) {
		Ok(job) => Ok(Json(Response {
			meta: meta_for_queue_item(&state.config, &routes, &job),
			response: job,
		})),
		Err(error) => Err(format!(
			"Unable to fetch jobs for repository {}. {}",
			repository, error
		)),
	}
}

#[derive(Debug)]
pub struct StaticAsset(PathBuf, Option<String>);

impl Responder<'static> for StaticAsset {
	fn respond_to(self, _req: &Request) -> Result<rocket::response::Response<'static>, Status> {
		if let Some(content) = self.1 {
			let mut response = rocket::response::Response::build();
			response.sized_body(Cursor::new(content));

			if let Some(extension) = self.0.extension() {
				if let Some(content_type) =
					ContentType::from_extension(&extension.to_string_lossy())
				{
					response.header(content_type);
				}
			}

			response.ok()
		} else {
			// TODO Handle properly
			Err(Status::NotFound)
		}
	}
}

#[get("/static/<file..>")]
pub fn get_static_asset(file: PathBuf) -> StaticAsset {
	if let Some(asset) = StaticAssets::get(file.to_str().unwrap()) {
		StaticAsset(
			file,
			Some(std::str::from_utf8(asset.as_ref()).unwrap().into()),
		)
	} else {
		StaticAsset(file, None)
	}
}

#[get("/swagger/<file..>")]
pub fn get_swagger_asset(file: PathBuf) -> StaticAsset {
	if let Some(asset) = ApiDefinitionUi::get(file.to_str().unwrap()) {
		StaticAsset(
			file,
			Some(std::str::from_utf8(asset.as_ref()).unwrap().into()),
		)
	} else {
		StaticAsset(file, None)
	}
}

#[get("/swagger")]
pub fn swagger() -> Redirect {
	Redirect::to("/swagger/index.html")
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
		allowed_methods: vec![Method::Get, Method::Post]
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

pub fn start_server(persisted_config: PersistedConfig) -> Result<(), Error> {
	let app_state = AppState::from(persisted_config.clone());

	// let tmp_user = crate::model::NewUserRecord { username: "admin".into(), password: "admin".into() };
	// let users = crate::model::Users::new(app_state.config.clone());
	// users.create(tmp_user);

	let http_config = Config::build(Environment::Production)
		// This should never use cookies though?
		.secret_key(encode(&nanoid::generate(32)))
		.address(&persisted_config.network_host)
		.port(persisted_config.port)
		.workers(1)
		.keep_alive(0)
		.finalize();

	match http_config {
		Ok(config) => {
			let routes = routes![
				get_config,
				// notify_with_secret,
				notify,
				// notify_with_secret_with_data,
				notify_with_data,
				notify_github,
				repositories,
				repository,
				jobs,
				job,
				log_output,
				login,
				get_static_asset,
				get_swagger_asset,
				swagger,
			];

			let route_map: RouteMap = Routes::new(&routes).into();

			// Rocket log formatting makes syslog output messy
			env::set_var("ROCKET_CLI_COLORS", "off");

			let server = rocket::custom(config)
				.attach(create_cors_options())
				.manage(app_state)
				.manage(route_map)
				.register(catchers![not_found_handler])
				.mount("/", routes);

			server.launch();
		}
		Err(error) => return Err(format_err!("Invalid HTTP configuration: {}", error)),
	};

	Ok(())
}
