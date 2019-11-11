use std::collections::HashMap;
use std::fs::read_to_string;
use std::str;
use serde_derive::{Serialize, Deserialize};
use failure::{Error, format_err};
use directories::ProjectDirs;
use secstr::SecStr;

use crate::{AppState};

#[derive(Deserialize, Default, Serialize, Debug, Clone)]
pub struct PersistedConfig {
	pub secret: String,
	pub data_dir: String,
	pub site_url: Option<String>,
	pub network_host: String,
	pub port: u16,
	pub log_to_syslog: bool,
	pub authentication_type: AuthenticationType,
	pub users: HashMap<String, User>,
	#[serde(default)]
	pub repositories: Vec<Repository>,
}

#[derive(Deserialize, Default, Serialize, Debug, Clone)]
pub struct User {
	pub username: String,
	pub password: String,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
	pub secret: SecStr,
	pub data_dir: String,
	pub network_host: String,
	pub site_url: String,
	pub port: u16,
	pub log_to_syslog: bool,
	pub authentication_type: AuthenticationType,
	pub users: HashMap<String, User>,
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
    fn default() -> Self { Self::Git(GitTrigger::Head(vec!["master".into()])) }
}

pub fn app_config_path() -> String {
	let project_dirs = ProjectDirs::from("org", "littleci", "LittleCI").unwrap();
	let file_path = format!("{}/Settings.json", project_dirs.config_dir().to_str().unwrap());
	file_path
}

pub fn load_app_config() -> Result<PersistedConfig, Error> {
	let file = read_to_string(app_config_path())?;
	let persisted_config: PersistedConfig = serde_json::from_str(&file).unwrap();
	Ok(persisted_config)
}

pub fn get_secret() -> Result<String, Error> {
	match load_app_config() {
		Ok(persisted_config) => {
			let app_state = AppState::from(persisted_config);
			let s = app_state.config.secret.unsecure();
			if let Err(err) = str::from_utf8(s) {
				eprintln!("{}", err);
			}
			Ok(str::from_utf8(s).unwrap().into())
		},
		Err(_) => Err(format_err!("No configuration found. Please configure LittleCI first.")),
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthenticationType {
	NoAuthentication,
	Simple,
}

impl Default for AuthenticationType {
	fn default() -> Self { Self::Simple }
}
