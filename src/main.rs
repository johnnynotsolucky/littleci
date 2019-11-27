#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate diesel;

use argon2::{self, Config, ThreadMode, Variant, Version};
use clap::{clap_app, value_t, ArgMatches};
use directories::ProjectDirs;
use failure::{format_err, Error};
use fern::colors::{Color, ColoredLevelConfig};
use regex::Regex;
use secstr::SecStr;
use sha3::{Digest, Sha3_256};
use std::collections::HashMap;
use std::convert::{From, Into};
use std::fmt::Write;
use std::fs::{self, create_dir_all};
use std::process;
use std::sync::Arc;

mod config;
mod model;
mod queue;
mod server;
mod util;

use crate::config::{
	app_config_path, load_app_config, AppConfig, AuthenticationType, PersistedConfig,
};
use crate::queue::{QueueManager, QueueService};
use crate::server::start_server;

#[allow(unused_imports)]
use log::{debug, error, info, warn};

const DEFAULT_PORT: u16 = 8000;

pub const ALPHA_NUMERIC: [char; 62] = [
	'0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i',
	'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', 'A', 'B',
	'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U',
	'V', 'W', 'X', 'Y', 'Z',
];

#[derive(Debug, Clone)]
pub struct HashedValue(String);

impl HashedValue {
	pub fn new(val: &str) -> Self {
		let mut hasher = Sha3_256::new();
		hasher.input(val.as_bytes());
		let signature_bytes = hasher.result();
		let mut hashed = String::new();
		for b in signature_bytes {
			write!(&mut hashed, "{:X}", b).expect("Unable to generate hashed value");
		}
		hashed = hashed.to_lowercase();
		HashedValue(hashed)
	}
}

impl Into<String> for HashedValue {
	fn into(self) -> String {
		self.0
	}
}

#[derive(Debug, Clone)]
pub struct HashedPassword(String);

impl HashedPassword {
	pub fn new(password: &str, salt: &str) -> Self {
		let config = Config {
			variant: Variant::Argon2id,
			version: Version::Version13,
			mem_cost: 4096,
			time_cost: 3,
			lanes: 1,
			thread_mode: ThreadMode::Sequential,
			secret: &[],
			ad: &[],
			hash_length: 32,
		};
		let encoded = argon2::hash_encoded(password.as_bytes(), salt.as_bytes(), &config).unwrap();
		HashedPassword(encoded)
	}

	pub fn verify(input_password: &str, stored_password: &str) -> bool {
		match argon2::verify_encoded(&input_password, stored_password.as_bytes()) {
			Ok(result) => result,
			Err(error) => {
				warn!("Could not verify password: {}", error);
				false
			}
		}
	}
}

impl Into<String> for HashedPassword {
	fn into(self) -> String {
		self.0
	}
}

#[derive(Debug, Clone)]
pub struct AppState {
	config: Arc<AppConfig>,
	queue_manager: Arc<QueueManager>,
	queues: Arc<HashMap<String, QueueService>>,
}

impl From<PersistedConfig> for AppState {
	fn from(configuration: PersistedConfig) -> Self {
		let secret: String = HashedValue::new(&configuration.secret).into();

		let config = AppConfig {
			secret: SecStr::from(secret.clone()),
			data_dir: configuration.data_dir,
			network_host: configuration.network_host.clone(),
			site_url: configuration.site_url.unwrap_or(configuration.network_host),
			port: configuration.port,
			log_to_syslog: configuration.log_to_syslog,
			authentication_type: configuration.authentication_type,
		};

		let queue_manager = QueueManager::new(Arc::new(config.clone()));

		Self {
			config: Arc::new(config),
			queue_manager: Arc::new(queue_manager),
			queues: Arc::new(HashMap::new()),
		}
	}
}

fn generate_config(matches: &ArgMatches) -> Result<String, Error> {
	let secret = nanoid::custom(16, &nanoid::alphabet::SAFE);

	let project_dirs = match ProjectDirs::from("dev", "tyrone", "littleci") {
		Some(project_dirs) => project_dirs,
		None => return Err(format_err!("Invalid $HOME path")),
	};

	let default_data_dir = String::from(project_dirs.data_dir().to_str().unwrap());
	let data_dir = matches.value_of("DATA_DIR").unwrap_or(&default_data_dir);

	let network_host = matches
		.value_of("NETWORK_HOST")
		.unwrap_or("0.0.0.0")
		.to_owned();
	let site_url = matches
		.value_of("SITE_URL")
		.map(|site_url| site_url.to_owned());

	let port = value_t!(matches.value_of("PORT"), u16).unwrap_or(DEFAULT_PORT);

	let log_to_syslog = matches.is_present("SYSLOG");

	let persisted_config = PersistedConfig {
		secret,
		data_dir: data_dir.to_string(),
		network_host,
		site_url,
		port,
		log_to_syslog,
		authentication_type: AuthenticationType::Simple,
	};

	let json = serde_json::to_string_pretty(&persisted_config);

	match json {
		Ok(json) => {
			let config_dir = String::from(project_dirs.config_dir().to_str().unwrap());
			create_dir_all(&config_dir)?;
			let file_path = format!("{}/Settings.json", config_dir);
			fs::write(&file_path, json)?;
			Ok(format!("Settings written to {}", file_path))
		}
		Err(error) => Err(format_err!("Unable to save config. {}", error)),
	}
}

/// Convert a string to an alphanumeric kebab-cased string.
pub fn kebab_case(original: &str) -> String {
	// Match groups of alphanumeric characters
	let re = Regex::new(r"([A-Za-z0-9])+").unwrap();

	// Match and add all the filtered groups into into a Vec
	let mut parts: Vec<&str> = Vec::new();
	for mat in re.find_iter(&original) {
		parts.push(mat.as_str());
	}

	// Generate kebab-cased string
	parts.join("-").to_lowercase()
}

fn setup_logger(log_to_syslog: bool) -> Result<(), Error> {
	let colors_line = ColoredLevelConfig::new()
		.error(Color::Red)
		.warn(Color::Yellow)
		.info(Color::Green)
		.debug(Color::White)
		.trace(Color::BrightBlack);
	let colors_level = colors_line.clone().info(Color::Green);

	let mut log_config = fern::Dispatch::new()
		.level(log::LevelFilter::Debug)
		.level_for("launch_", log::LevelFilter::Warn)
		.level_for("launch", log::LevelFilter::Warn)
		.level_for("rocket::rocket", log::LevelFilter::Info)
		.level_for("hyper::server", log::LevelFilter::Warn)
		.level_for("_", log::LevelFilter::Warn)
		.chain(
			fern::Dispatch::new()
				.format(move |out, message, record| {
					out.finish(format_args!(
						"{color_line}[{date}][{target}][{level}{color_line}] {message}\x1B[0m",
						color_line = format_args!(
							"\x1B[{}m",
							colors_line.get_color(&record.level()).to_fg_str()
						),
						date = chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
						target = record.target(),
						level = colors_level.color(record.level()),
						message = message,
					))
				})
				.chain(std::io::stdout()),
		);

	if cfg!(linux) {
		if log_to_syslog {
			log_config = configure_syslog(log_config);
		}
	}

	log_config.apply()?;

	Ok(())
}

#[cfg(target_os = "linux")]
fn configure_syslog(log_config: fern::Dispatch) -> fern::Dispatch {
	let syslog_formatter = syslog::Formatter3164 {
		facility: syslog::Facility::LOG_USER,
		hostname: None,
		process: "littleci".to_owned(),
		pid: process::id() as i32,
	};

	log_config.chain(
		fern::Dispatch::new()
			.level(log::LevelFilter::Info)
			.chain(syslog::unix(syslog_formatter).unwrap()),
	)
}

#[cfg(not(target_os = "linux"))]
fn configure_syslog(log_config: fern::Dispatch) -> fern::Dispatch {
	log_config
}

fn main() {
	let command_matches = clap_app!(LittleCI =>
		(version: "0.1.0")
		(author: "Tyrone Tudehope")
		(about: "The littlest CI")
		(@subcommand configure =>
			(about: "Pre-configure LittleCI with defaults")
			(@arg NETWORK_HOST: -h --network_host "Bind to this host or IP address. Default 0.0.0.0")
			(@arg PORT: -p --port +takes_value +takes_value "TCP Port to bind to")
			(@arg SITE_URL: -U "External URL which LittleCI can be accessed from. Defaults to NETWORK_HOST")
			(@arg DATA_DIR: -l --data_dir +takes_value "Location for application output")
			(@arg SYSLOG: --syslog "Whether or not messages should be logged to syslog")
		)
		(@subcommand config_path =>
			(about: "Returns the full path to LittleCI config")
		)
		(@subcommand serve =>
			(about: "Launch LittleCI's HTTP server")
		)
	)
	.get_matches();

	if let Some(matches) = command_matches.subcommand_matches("configure") {
		match generate_config(matches) {
			Ok(message) => println!("{}", message),
			Err(error) => eprintln!("Config generation failed. {}", error),
		}
	}

	if command_matches.subcommand_matches("config_path").is_some() {
		println!("{}", app_config_path());
	}

	if command_matches.subcommand_matches("serve").is_some() {
		match load_app_config() {
			Ok(persisted_config) => {
				setup_logger(persisted_config.log_to_syslog)
					.expect("Failed to initialize the logger");
				if let Err(error) = start_server(persisted_config) {
					eprintln!("Unable to start server. {}", error);
				}
			}
			Err(_) => eprintln!("No configuration found. Please configure LittleCI first."),
		}
	}
}
