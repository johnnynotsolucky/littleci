use std::collections::HashMap;
use std::sync::Arc;
use rocket::Route as RocketRoute;
use serde_derive::{Serialize};

use crate::config::{Particle, Trigger, AppConfig};
use crate::queue::QueueItem;

type Segment = Vec<(String, bool)>;

#[derive(Debug, Clone)]
pub struct Route {
    pub base: String,
    pub segments: Segment,
}

impl Route {
    #[allow(dead_code)]
    pub fn url(&self, parts: Vec<(&str, &str)>) -> String {
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

pub type RouteMap = Arc<HashMap<String, Route>>;

#[derive(Debug, Clone)]
pub struct Routes(RouteMap);

impl Routes {
    pub fn new(routes: &[RocketRoute]) -> Self {
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
	pub fn new(name: &str, particle: &Arc<Particle>) -> Self {
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


pub fn meta_for_particle(app_config: &AppConfig, routes: &RouteMap, particle: &ParticleResponse) -> ResponseMeta {
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

pub fn meta_for_queue_item(app_config: &AppConfig, routes: &RouteMap, queue_item: &QueueItem) -> ResponseMeta {
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


