use chrono::NaiveDateTime;
use serde_derive::Serialize;
use std::collections::HashMap;
use std::str;
use std::sync::Arc;

use crate::config::{AppConfig, Trigger};
use crate::model::repositories::Repository;
use crate::model::users::User;
use crate::util::serialize_date;

#[allow(unused_imports)]
use log::{debug, error, info, warn};

#[derive(Serialize, Debug, Clone)]
pub struct ErrorResponse {
	pub message: String,
}

impl ErrorResponse {
	pub fn new(message: String) -> Self {
		Self { message }
	}
}

#[derive(Serialize, Debug, Clone)]
pub struct Response<T> {
	#[serde(flatten)]
	pub response: T,
}

#[derive(Serialize, Debug, Clone)]
pub struct UserResponse {
	pub id: String,
	pub username: String,
	#[serde(serialize_with = "serialize_date")]
	pub created_at: NaiveDateTime,
	#[serde(serialize_with = "serialize_date")]
	pub updated_at: NaiveDateTime,
}

impl From<User> for UserResponse {
	fn from(user: User) -> Self {
		Self {
			id: user.id,
			username: user.username,
			created_at: user.created_at,
			updated_at: user.updated_at,
		}
	}
}

#[derive(Serialize, Debug, Clone)]
pub struct RepositoryResponse {
	pub id: String,
	pub slug: String,
	pub name: String,
	pub run: String,
	pub working_dir: Option<String>,
	pub variables: HashMap<String, String>,
	pub triggers: Vec<Trigger>,
	pub webhooks: Vec<String>,
	pub secret: String,
}

impl From<Repository> for RepositoryResponse {
	fn from(repository: Repository) -> Self {
		Self {
			id: repository.id,
			slug: repository.slug,
			name: repository.name,
			run: repository.run,
			working_dir: repository.working_dir,
			secret: repository.secret,
			variables: repository.variables,
			triggers: repository.triggers,
			webhooks: repository.webhooks,
		}
	}
}

#[derive(Serialize, Debug, Clone)]
pub struct AppConfigResponse {
	pub signature: String,
	pub config_path: String,
	pub working_dir: String,
	pub data_dir: String,
	pub network_host: String,
	pub site_url: String,
	pub port: u16,
}

impl From<Arc<AppConfig>> for AppConfigResponse {
	fn from(app_config: Arc<AppConfig>) -> AppConfigResponse {
		let signature = str::from_utf8(app_config.secret.unsecure()).unwrap().into();

		AppConfigResponse {
			signature,
			config_path: app_config.config_path.clone(),
			working_dir: app_config.working_dir.clone(),
			data_dir: app_config.data_dir.clone(),
			network_host: app_config.network_host.clone(),
			site_url: app_config.site_url.clone(),
			port: app_config.port.clone(),
		}
	}
}
