#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate diesel;

use argon2::{self, Config, ThreadMode, Variant, Version};
use clap::clap_app;
use failure::Error;
use fern::colors::{Color, ColoredLevelConfig};
use regex::Regex;
use secstr::SecStr;
use sha3::{Digest, Sha3_256};
use std::convert::{From, Into};
use std::env::current_dir;
use std::fmt::Write;
use std::path::Path;
use std::process;
use std::sync::Arc;

mod config;
mod model;
mod queue;
mod server;
mod util;

use crate::config::{load_app_config, AppConfig, PersistedConfig};
use crate::queue::QueueManager;
use crate::server::start_server;

#[allow(unused_imports)]
use log::{debug, error, info, warn};

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
}

impl From<PersistedConfig> for AppState {
	fn from(configuration: PersistedConfig) -> Self {
		let secret: String = HashedValue::new(&configuration.secret).into();

		let working_dir = Path::new(
			current_dir()
				.expect("Working directory is invalid")
				.to_str()
				.unwrap_or("./"),
		)
		.canonicalize()
		.expect("Working dir is invalid");

		let config_path = Path::new(&configuration.config_path)
			.canonicalize()
			.expect("Configuration path is invalid");

		let data_dir = match configuration.data_dir {
			Some(data_dir) => Path::new(&data_dir)
				.canonicalize()
				.expect("Data directory is invalid"),
			None => {
				let data_dir: String = match config_path.parent() {
					Some(parent) => parent.to_str().unwrap_or("./").into(),
					None => working_dir.to_str().expect("Working dir is invalid").into(),
				};

				Path::new(&data_dir)
					.canonicalize()
					.expect("Working directory is invalid")
			}
		};

		let config = AppConfig {
			secret: SecStr::from(secret.clone()),
			config_path: config_path
				.to_str()
				.expect("Configuration path is invalid")
				.into(),
			working_dir: working_dir
				.to_str()
				.expect("Configuration path is invalid")
				.into(),
			data_dir: data_dir.to_str().expect("Data directory is invalid").into(),
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
		}
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
		(@subcommand serve =>
			(about: "Launch LittleCI's HTTP server")
			(@arg CONFIG_FILE: --config +takes_value "Path to config file")
		)
	)
	.get_matches();

	if let Some(matches) = command_matches.subcommand_matches("serve") {
		let working_dir = current_dir().expect("Working directory is invalid");
		let working_dir = working_dir.to_str().unwrap_or("./");
		let config_path = matches.value_of("CONFIG_FILE").unwrap_or(working_dir);
		match load_app_config(config_path) {
			Ok(mut persisted_config) => {
				setup_logger(persisted_config.log_to_syslog)
					.expect("Failed to initialize the logger");

				persisted_config.config_path = config_path.into();
				let app_state = AppState::from(persisted_config.clone());
				if let Err(error) = start_server(app_state) {
					eprintln!("Unable to start server. {}", error);
				}
			}
			Err(_) => eprintln!("No configuration found. Please configure LittleCI first."),
		}
	}
}
