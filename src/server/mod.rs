use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::fs::read_to_string;
use crypto::digest::Digest;
use crypto::sha1::Sha1;
use rocket::http::{RawStr, Status};
use rocket::{Outcome, State, Route as RocketRoute, get, post, routes};
use rocket::config::{Config, Environment};
use rocket::request::{self, Request, FromRequest, FromParam};
use rocket_contrib::json::Json;
use failure::{Error, Fail, format_err};
use serde_derive::{Serialize, Deserialize};
use secstr::SecStr;
use base64::encode;

use crate::{AppState};
use crate::config::{Particle, Trigger, GitTrigger, AppConfig, PersistedConfig};
use crate::queue::{QueueItem, ArbitraryData, QueueManager};

#[allow(unused_imports)]
use log::{debug, info, warn, error};

mod github;
use github::{GitHubPayload, GitReference, GitHubSecret};

#[derive(Serialize, Debug, Clone)]
pub struct Response<T> {
	#[serde(flatten)]
	pub response: T,
	#[serde(rename = "_meta")]
	pub meta: ResponseMeta,
}

#[derive(Serialize, Debug, Clone)]
pub struct ParticleResponse {
	#[serde(rename = "particle")]
	pub name: String,
	pub command: String,
	pub working_dir: Option<String>,
	pub variables: HashMap<String, String>,
	pub triggers: Vec<Trigger>,

}

impl ParticleResponse {
	fn new(name: &str, particle: &Arc<Particle>) -> Self {
		Self {
			name: name.to_owned(),
			command: particle.command.clone(),
			working_dir: particle.working_dir.clone(),
			variables: particle.variables.clone(),
			triggers: particle.triggers.clone(),
		}
	}
}

#[derive(Serialize, Debug, Clone)]
pub struct ResponseMeta(HashMap<String, String>);

impl From<Vec<(&str, &str)>> for ResponseMeta {
	fn from(items: Vec<(&str, &str)>) -> Self {
		let mut mapped = HashMap::new();

		for (key, value) in items.iter() {
			let key = *key;
			let value = *value;
			mapped.insert(key.into(), value.into());
		}

		Self(mapped)
	}
}

pub struct SecretKey;

#[derive(Fail, Debug, Clone)]
pub enum SecretKeyError {
    #[fail(display = "Secret key was not found")]
    Missing,
    #[fail(display = "Secret key is invalid")]
    Invalid,
}

fn secret_key_is_valid(key: &str, state: &AppState) -> bool {
    let signature = SecStr::from(key);
    let state_signature = &state.config.signature;

    &signature == state_signature
}

impl<'a, 'r> FromRequest<'a, 'r> for SecretKey {
    type Error = SecretKeyError;

    fn from_request(request: &'a Request<'r>) -> request::Outcome<Self, SecretKeyError> {
        let secret_key = request.headers().get("x-secret-key").next();
        match secret_key {
            Some(secret_key) => {
                let state = request.guard::<State<AppState>>().unwrap();
                if secret_key_is_valid(&secret_key, &state) {
                    Outcome::Success(SecretKey)
                } else {
                    Outcome::Failure((Status::BadRequest, SecretKeyError::Invalid))
                }
            },
            _ => Outcome::Failure((Status::BadRequest, SecretKeyError::Missing))
        }
    }
}

#[derive(Deserialize, Debug)]
pub enum LogType {
    Stdout,
    Stderr,
}

impl Into<String> for LogType {
    fn into(self) -> String {
        match self {
            Self::Stdout => "stdout".into(),
            Self::Stderr => "stderr".into(),
        }
    }
}

impl<'a> FromParam<'a> for LogType {
    type Error = Error;

    fn from_param(param: &'a RawStr) -> Result<Self, Self::Error> {
        let param = param.as_str();

        match param {
            "stdout" => Ok(LogType::Stdout),
            "stderr" => Ok(LogType::Stderr),
            _ => Err(format_err!("Invalid log type")),
        }
    }
}

fn meta_for_particle(app_config: &AppConfig, routes: &RouteMap, particle: &ParticleResponse) -> ResponseMeta {
	let identity_url = format!("{}{}", &app_config.site_url,
		routes
			.get("particle")
			.unwrap()
			.url(vec![
				("particle", &particle.name),
			])
	);

	let jobs_url = format!("{}{}", &app_config.site_url,
		routes
			.get("jobs")
			.unwrap()
			.url(vec![
				("particle", &particle.name),
			])
	);

	ResponseMeta::from(vec![
		("identity", &identity_url[..]),
		("jobs", &jobs_url[..]),
	])
}

fn meta_for_queue_item(app_config: &AppConfig, routes: &RouteMap, queue_item: &QueueItem) -> ResponseMeta {
	let identity_url = format!("{}{}", &app_config.site_url,
		routes
			.get("job")
			.unwrap()
			.url(vec![
				("particle", &queue_item.particle),
				("id", &queue_item.id),
			])
	);

	let log_output = routes.get("log_output").unwrap();
	let stdout_url = format!("{}{}", &app_config.site_url,
		log_output.clone()
			.url(vec![
				("particle", &queue_item.particle),
				("id", &queue_item.id),
				("log", "stdout"),
			])
	);

	let stderr_url = format!("{}{}", &app_config.site_url,
		log_output.clone()
			.url(vec![
				("particle", &queue_item.particle),
				("id", &queue_item.id),
				("log", "stderr"),
			])
	);

	ResponseMeta::from(vec![
		("identity", &identity_url[..]),
		("stdout", &stdout_url[..]),
		("stderr", &stderr_url[..]),
	])
}

fn notify_new_job(particle: &str, values: ArbitraryData, state: &AppState, routes: &RouteMap) -> Result<Response<QueueItem>, String> {
    match state.queue_manager.push(particle, values) {
        Ok(item) => {
			Ok(Response {
				meta: meta_for_queue_item(&state.config, &routes, &item),
				response: item,
			})
		},
        Err(error) => Err(format!("{}", error)),
    }
}

fn notify_job(particle: &RawStr, values: ArbitraryData, state: &AppState, routes: &RouteMap) -> Result<Json<Response<QueueItem>>, String> {
	match notify_new_job(particle.as_str(), values, state, routes) {
		Ok(job) => Ok(Json(job)),
		Err(error) => Err(error),
	}
}

#[get("/notify/<particle>")]
pub fn notify(particle: &RawStr, _secret_key: SecretKey, state: State<AppState>, routes: State<RouteMap>)
	-> Result<Json<Response<QueueItem>>, String>
{
	notify_job(particle, ArbitraryData::new(HashMap::new()), state.inner(), routes.inner())
}

#[post("/notify/<particle>", format = "json", data = "<data>")]
pub fn notify_with_data(particle: &RawStr, data: Json<ArbitraryData>, _secret_key: SecretKey, state: State<AppState>, routes: State<RouteMap>)
	-> Result<Json<Response<QueueItem>>, String>
{
	notify_job(particle, data.into_inner(), state.inner(), routes.inner())
}

#[derive(Serialize, Clone, Debug)]
pub enum JobOrSkipped {
	Skipped(String),
	Job(Response<QueueItem>),
}

#[post("/notify/<particle>/github", format = "json", data = "<payload>")]
pub fn notify_github(
	particle: &RawStr,
	payload: Json<GitHubPayload>,
	_github_secret: GitHubSecret,
	state: State<AppState>,
	routes: State<RouteMap>
	) -> Result<Json<JobOrSkipped>, String> {

	let particle_name = particle.as_str();
	let particle = match state.particles.get(particle_name) {
		Some(particle) => particle,
		None => return Err(format!("Particle `{}` does not exist", particle)),
	};

	let mut should_skip = true;
	let triggers = particle.triggers.clone();
	for trigger in triggers.into_iter() {
		match trigger {
			Trigger::Any => {
				debug!("Matched any trigger for particle {}", particle_name);
				should_skip = false;
				break;
			},
			Trigger::Git(GitTrigger::Any) => {
				debug!("Matched any git trigger for particle {}", particle_name);
				should_skip = false;
				break;
			},
			Trigger::Git(GitTrigger::Tag) => {
				debug!("Trigger tag");
				if let GitReference::Tag(_) = &payload.reference {
					debug!("Matched tag trigger for particle {}", particle_name);
					should_skip = false;
				}
			},
			Trigger::Git(GitTrigger::Head(refs)) => {
				for trigger_ref in refs.iter() {
					if let GitReference::Head(payload_ref) = &payload.reference {
						if *trigger_ref == *payload_ref {
							debug!("Matched head trigger {} for particle {}", &trigger_ref, particle_name);
							should_skip = false;
						}
					}
				}
			},
		}
	}

	if should_skip {
		debug!("Skipping job for particle {}", particle_name);
		Ok(Json(JobOrSkipped::Skipped("Trigger rules not matched. No job queued".into())))
	} else {
		debug!("Notifying new job for particle {}", particle_name);
		match notify_new_job(
			particle_name,
			ArbitraryData::from(payload.into_inner()),
			state.inner(),
			routes.inner()
		) {
			Ok(response) => Ok(Json(JobOrSkipped::Job(response))),
			Err(error) => Err(error)
		}
	}
}


#[get("/notify/<particle>/<signature>")]
pub fn notify_with_signature(
	signature: &RawStr,
	particle: &RawStr,
	state: State<AppState>,
	routes: State<RouteMap>
	) -> Result<Json<Response<QueueItem>>, String>
{
    if secret_key_is_valid(signature.as_str(), &state) {
			notify_job(particle, ArbitraryData::new(HashMap::new()), state.inner(), routes.inner())
    } else {
        Err("Invalid Signature".into())
    }
}

#[post("/notify/<particle>/a/<signature>", format = "json", data = "<data>")]
pub fn notify_with_signature_with_data(
	signature: &RawStr,
	particle: &RawStr,
	data: Json<ArbitraryData>,
	state: State<AppState>,
	routes: State<RouteMap>
	) -> Result<Json<Response<QueueItem>>, String>
{
    if secret_key_is_valid(signature.as_str(), &state) {
		notify_job(particle, data.into_inner(), state.inner(), routes.inner())
	} else {
        Err("Invalid Signature".into())
    }
}

#[get("/particles")]
pub fn particles(state: State<AppState>, routes: State<RouteMap>)
	-> Result<Json<Vec<Response<ParticleResponse>>>, String>
{
	Ok(
		Json(
			state.particles.iter()
				.map(|(key, particle)| {
					let particle = ParticleResponse::new(key, particle);
					Response {
						meta: meta_for_particle(&state.config, &routes, &particle),
						response: particle,
					}
				})
				.collect()
		)
	)
}

#[get("/particles/<particle>")]
pub fn particle(particle: &RawStr, state: State<AppState>, routes: State<RouteMap>)
	-> Result<Json<Response<ParticleResponse>>, String>
{
    let particle_name = particle.as_str();
	match state.particles.get(particle_name) {
		Some(particle) => {
			let particle = ParticleResponse::new(particle_name, particle);
			Ok(Json(Response {
				meta: meta_for_particle(&state.config, &routes, &particle),
				response: particle,
			}))
		},
		None => return Err(format!("Particle `{}` does not exist", particle)),
	}
}

#[get("/particles/<particle>/jobs")]
pub fn jobs(particle: &RawStr, state: State<AppState>, routes: State<RouteMap>)
	-> Result<Json<Vec<Response<QueueItem>>>, String>
{
    let particle = particle.as_str();
    let particle = {
        match state.particles.get(particle) {
            Some(_) => particle,
            None => return Err(format!("Particle `{}` does not exist", particle)),
        }
    };

    match state.queue_manager.all(&particle) {
        Ok(jobs) => Ok(Json(jobs
				.into_iter()
				.map(|job| {
					Response {
						meta: meta_for_queue_item(&state.config, &routes, &job),
						response: job,
					}
				})
				.collect())),
        Err(error) => Err(format!("Unable to fetch jobs for particle {}. {}", particle, error)),
    }
}

#[get("/particles/<particle>/jobs/<id>/logs/<log>")]
pub fn log_output(particle: &RawStr, id: &RawStr, log: LogType, state: State<AppState>) -> Result<String, String> {
    let particle = particle.as_str();
    let particle = {
        match state.particles.get(particle) {
            Some(_) => particle,
            None => return Err(format!("Particle `{}` does not exist", particle)),
        }
    };

    let id = id.as_str();

    match state.queue_manager.job(&particle, &id) {
        Ok(job) => {
            let log: String = log.into();
            let log_output = read_to_string(format!("{}/jobs/{}/{}.log", &state.config.data_dir, &job.id, &log));
            match log_output {
                Ok(log_output) => Ok(log_output),
                Err(error) => Err(format!("Unable to read log file {} for job {}. {}", &log, &id, error)),
            }
        },
        Err(error) => Err(format!("Unable to fetch jobs for particle {}. {}", particle, error)),
    }
}

#[get("/particles/<particle>/jobs/<id>")]
pub fn job(particle: &RawStr, id: &RawStr, state: State<AppState>, routes: State<RouteMap>) -> Result<Json<Response<QueueItem>>, String> {
    let particle = particle.as_str();
    let particle = {
        match state.particles.get(particle) {
            Some(_) => particle,
            None => return Err(format!("Particle `{}` does not exist", particle)),
        }
    };

    let id = id.as_str();

    match state.queue_manager.job(&particle, &id) {
        Ok(job) => {
			Ok(Json(Response {
				meta: meta_for_queue_item(&state.config, &routes, &job),
				response: job,
			}))
		},
        Err(error) => Err(format!("Unable to fetch jobs for particle {}. {}", particle, error)),
    }
}

type Segment = Vec<(String, bool)>;

#[derive(Debug, Clone)]
pub struct Route {
    pub base: String,
    pub segments: Segment,
}

impl Route {
    #[allow(dead_code)]
    fn url(&self, parts: Vec<(&str, &str)>) -> String {
		let segments = self.segments.clone();

		let url: String = segments.into_iter().fold("".into(), move |url, (segment, is_dynamic)| {
			if is_dynamic {
				match parts.iter().find(|(s, _)| *s == segment) {
					Some((_, value)) => format!("{}/{}", url, value),
					None => panic!("Invalid URL segment"),
				}
			} else {
				format!("{}/{}", url, segment)
			}
		});

		url
    }
}

impl From<&RocketRoute> for Route {
    fn from(route: &RocketRoute) -> Route {
        let mut segments = Vec::new();
        segments.reserve_exact(route.uri.segments().count());
        route.uri.segments()
            .enumerate()
            .for_each(|segment| {
                let (index, segment) = segment;
                let (segment, is_dynamic) = match (segment.chars().next(), segment.chars().last()) {
                    (Some('<'), Some('>')) => (String::from(&segment[1..segment.len() - 1]), true),
                    _ => (segment.to_owned(), false),
                };
                segments.insert(index, (segment, is_dynamic));
            });

        Route {
            base: route.base.path().to_owned(),
            segments,
        }
    }
}

type RouteMap = Arc<HashMap<String, Route>>;

#[derive(Debug, Clone)]
struct Routes(RouteMap);

impl Routes {
    fn new(routes: &[RocketRoute]) -> Self {
        let mut route_map = HashMap::new();

        routes.iter().for_each(|route| {
            route_map.insert(route.name.unwrap().to_owned(), Route::from(route));
        });

        Routes(Arc::new(route_map))
    }
}

impl Into<RouteMap> for Routes {
    fn into(self) -> RouteMap {
        self.0
    }
}

impl From<PersistedConfig> for AppState {
    fn from(configuration: PersistedConfig) -> Self {
        let mut hasher = Sha1::new();
        hasher.input_str(&configuration.secret);
        let signature = hasher.result_str();

        let config = AppConfig {
            signature: SecStr::from(signature),
            data_dir: configuration.data_dir,
            network_host: configuration.network_host.clone(),
            site_url: configuration.site_url.unwrap_or(configuration.network_host),
            port: configuration.port,
            log_to_syslog: configuration.log_to_syslog,
        };

        let mut particles = HashMap::new();
        for (name, particle) in configuration.particles.iter() {
            particles.insert(name.to_owned(), Arc::new(particle.clone()));
        }

        let queue_manager = QueueManager::new(Arc::new(config.clone()), &particles);

        Self {
            config: Arc::new(config),
            queue_manager: Arc::new(queue_manager),
            particles: Arc::new(particles),
            queues: Arc::new(HashMap::new()),
        }
    }
}

pub fn start_server(persisted_config: PersistedConfig) -> Result<(), Error> {
    let app_state = AppState::from(persisted_config.clone());

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
                notify,
				notify_with_data,
                notify_with_signature,
				notify_with_signature_with_data,
				notify_github,
				particles,
				particle,
                jobs,
                log_output,
                job,
            ];

            let route_map: RouteMap = Routes::new(&routes).into();

            // Rocket log formatting makes syslog output messy
            env::set_var("ROCKET_CLI_COLORS", "off");

            let server = rocket::custom(config)
                .manage(app_state)
                .manage(route_map)
                .mount("/", routes);

            server.launch();
        },
        Err(error) => return Err(format_err!("Invalid HTTP configuration: {}", error)),
    };

    Ok(())
}

