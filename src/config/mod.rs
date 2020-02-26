use failure::Error;
use secstr::SecStr;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::fs::read_to_string;
use std::path::Path;
use std::str;

#[allow(unused_imports)]
use log::{debug, error, info, warn};

#[derive(Deserialize, Default, Serialize, Debug, Clone)]
pub struct PersistedConfig {
	pub secret: String,
	#[serde(default, skip_serializing)]
	pub config_path: String,
	pub data_dir: Option<String>,
	pub network_host: String,
	pub port: u16,
	#[serde(default)]
	pub authentication_type: AuthenticationType,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
	pub secret: SecStr,
	pub config_path: String,
	pub working_dir: String,
	pub data_dir: String,
	pub network_host: String,
	pub port: u16,
	pub authentication_type: AuthenticationType,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct Repository {
	pub name: String,
	pub run: String,
	pub working_dir: Option<String>,
	pub webhooks: Option<Vec<String>>,
	#[serde(default)]
	pub variables: HashMap<String, String>,
	#[serde(default)]
	pub triggers: Vec<Trigger>,
	#[serde(skip)]
	pub secret: Option<SecStr>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum GitTrigger {
	#[serde(rename = "any")]
	Any,
	#[serde(rename = "head")]
	Head(Vec<String>),
	#[serde(rename = "tag")]
	Tag,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum Trigger {
	#[serde(rename = "any")]
	Any,
	#[serde(rename = "git")]
	Git(GitTrigger),
}

impl Default for Trigger {
	fn default() -> Self {
		Self::Git(GitTrigger::Head(vec!["master".into()]))
	}
}

pub fn load_app_config(config_path: &str) -> Result<PersistedConfig, Error> {
	let path = Path::new(config_path);

	// If config_path is a dir, either load littleci.json or create a default configuration file at
	// config_path location
	let persisted_config: PersistedConfig = if path.is_dir() {
		let default_config_path = format!("{}/littleci.json", config_path);
		let path = Path::new(&default_config_path);

		// First try the littleci.json file if it exists
		if path.is_file() {
			let data = read_to_string(default_config_path.clone())?;
			let mut persisted_config: PersistedConfig = serde_json::from_str(&data)?;
			persisted_config.config_path = default_config_path;
			persisted_config
		} else {
			let persisted_config = PersistedConfig {
				secret: nanoid::custom(64, &crate::ALPHA_NUMERIC),
				network_host: "0.0.0.0".into(),
				port: 8000,
				data_dir: Some(config_path.into()),
				config_path: default_config_path.clone(),
				..Default::default()
			};

			let json = serde_json::to_string_pretty(&persisted_config)?;
			fs::write(&default_config_path, json)?;

			persisted_config
		}
	} else {
		// Otherwise try read the file provided
		let data = read_to_string(config_path)?;
		let mut persisted_config: PersistedConfig = serde_json::from_str(&data)?;
		persisted_config.config_path = config_path.into();
		persisted_config
	};

	Ok(persisted_config)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthenticationType {
	NoAuthentication,
	Simple,
}

impl Default for AuthenticationType {
	fn default() -> Self {
		Self::Simple
	}
}
