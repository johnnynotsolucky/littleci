use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;
use std::fs::read_to_string;
use std::str;
use serde::{Deserializer};
use serde::de::{Visitor, MapAccess};
use serde_derive::{Serialize, Deserialize};
use failure::{Error, format_err};
use directories::ProjectDirs;
use secstr::SecStr;

use crate::{AppState, kebab_case};

type ParticleMap = HashMap<String, Particle>;

fn deserialize_particle_map<'de, D>(d: D) -> Result<ParticleMap, D::Error>
where D: Deserializer<'de>,
{
	match d.deserialize_map(ParticleMapVisitor::new()) {
		Ok(map) => {
			let mut sanitized = HashMap::new();
			map.iter().for_each(|(key, val)| {
				sanitized.insert(kebab_case(&key), val.clone());
			});

			Ok(sanitized)
		},
		Err(error) => Err(error),
	}
}

#[derive(Debug)]
struct ParticleMapVisitor {
	marker: PhantomData<fn() -> ParticleMap>,
}

impl ParticleMapVisitor {
	fn new() -> Self {
		ParticleMapVisitor {
			marker: PhantomData,
		}
	}
}

impl<'de> Visitor<'de> for ParticleMapVisitor {
	type Value = ParticleMap;

	fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		formatter.write_str("Map of particle data")
	}

	fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
	where
		M: MapAccess<'de>,
	{
		let mut map = HashMap::with_capacity(access.size_hint().unwrap_or(0));
		while let Some((key, value)) = access.next_entry()? {
			map.insert(key, value);
		}

		Ok(map)
	}
}

#[derive(Deserialize, Default, Serialize, Debug, Clone)]
pub struct PersistedConfig {
	pub secret: String,
	pub data_dir: String,
	pub site_url: Option<String>,
	pub network_host: String,
	pub port: u16,
	pub log_to_syslog: bool,
	#[serde(default)]
	#[serde(deserialize_with = "deserialize_particle_map")]
	pub particles: ParticleMap,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
	pub signature: SecStr,
	pub data_dir: String,
	pub network_host: String,
	pub site_url: String,
	pub port: u16,
	pub log_to_syslog: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct Particle {
	pub command: String,
	pub working_dir: Option<String>,
	pub webhooks: Option<Vec<String>>,
	#[serde(default)]
	pub variables: HashMap<String, String>,
	#[serde(default)]
	pub triggers: Vec<Trigger>,
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

pub fn get_hashed_signature() -> Result<String, Error> {
	match load_app_config() {
		Ok(persisted_config) => Ok(
			str::from_utf8(
				AppState::from(persisted_config).config.signature.unsecure()
			).unwrap().into()
		),
		Err(_) => Err(format_err!("No configuration found. Please configure LittleCI first.")),
	}
}
