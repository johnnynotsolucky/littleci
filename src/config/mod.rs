use failure::Error;
use secstr::SecStr;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::read_to_string;
use std::str;

#[derive(Deserialize, Default, Serialize, Debug, Clone)]
pub struct PersistedConfig {
	pub secret: String,
	#[serde(default)]
	pub config_path: String,
	pub data_dir: Option<String>,
	pub site_url: Option<String>,
	pub network_host: String,
	pub port: u16,
	pub authentication_type: AuthenticationType,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
	pub secret: SecStr,
	pub config_path: String,
	pub working_dir: String,
	pub data_dir: String,
	pub network_host: String,
	pub site_url: String,
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
	let file = read_to_string(config_path)?;
	let persisted_config: PersistedConfig = serde_json::from_str(&file).unwrap();
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
