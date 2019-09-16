#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use] extern crate diesel;

use std::sync::Arc;
use std::fs::{self, create_dir_all};
use std::process;
use std::collections::HashMap;
use std::convert::{Into, From};
use regex::Regex;
use fern::colors::{Color, ColoredLevelConfig};
use directories::ProjectDirs;
use clap::{clap_app, ArgMatches, value_t};
use failure::{Error, format_err};

mod model;
mod server;
mod config;
mod queue;
mod util;

use crate::server::start_server;
use crate::config::{
    app_config_path,
	load_app_config,
    get_hashed_signature,
    AppConfig,
    PersistedConfig,
    Particle,
	Trigger,
};
use crate::queue::{QueueManager, QueueService};

#[allow(unused_imports)]
use log::{debug, info, warn, error};

const DEFAULT_PORT: u16 = 8000;

#[derive(Debug, Clone)]
pub struct AppState {
    config: Arc<AppConfig>,
    particles: Arc<HashMap<String, Arc<Particle>>>,
    queue_manager: Arc<QueueManager>,
    queues: Arc<HashMap<String, QueueService>>,
}

fn generate_config(matches: &ArgMatches) -> Result<String, Error> {
    let secret = nanoid::custom(16, &nanoid::alphabet::SAFE);

    let project_dirs = match ProjectDirs::from("dev", "tyrone", "littleci") {
        Some(project_dirs) => project_dirs,
        None => return Err(format_err!("Invalid $HOME path")),
    };

    let default_data_dir = String::from(project_dirs.data_dir().to_str().unwrap());
    let data_dir = matches
        .value_of("DATA_DIR")
        .unwrap_or(&default_data_dir);

    let network_host = matches.value_of("NETWORK_HOST").unwrap_or("0.0.0.0").to_owned();
    let site_url = matches.value_of("SITE_URL").map(|site_url| site_url.to_owned());

    let port = value_t!(matches.value_of("PORT"), u16).unwrap_or(DEFAULT_PORT);

    let log_to_syslog = matches.is_present("SYSLOG");

    let persisted_config = PersistedConfig {
        secret,
        data_dir: data_dir.to_string(),
        network_host,
        site_url,
        port,
        log_to_syslog,
        particles: HashMap::new(),
    };

    let json = serde_json::to_string_pretty(&persisted_config);

    match json {
        Ok(json) => {
            let config_dir = String::from(project_dirs.config_dir().to_str().unwrap());
            create_dir_all(&config_dir)?;
            let file_path = format!("{}/Settings.json", config_dir);
            fs::write(&file_path, json)?;
            Ok(format!("Settings written to {}", file_path))
        },
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

fn add_particle_config(matches: &ArgMatches) -> Result<String, Error> {
    let project_dirs = match ProjectDirs::from("dev", "tyrone", "littleci") {
        Some(project_dirs) => project_dirs,
        None => return Err(format_err!("Invalid $HOME path.")),
    };

    let mut persisted_config = load_app_config()?;

	let triggers = vec![Trigger::Any];

    let particle = Particle {
        command: matches.value_of("COMMAND").unwrap().to_owned(),
        working_dir: match matches.value_of("WORKING_DIR") {
            Some(working_dir) => Some(working_dir.to_owned()),
            None => None,
        },
		triggers,
        ..Default::default()
    };

    let particle_name = kebab_case(&matches.value_of("PARTICLE_NAME").unwrap()).to_owned();

    persisted_config.particles.insert(particle_name, particle);

    let json = serde_json::to_string_pretty(&persisted_config)?;
    let config_dir = String::from(project_dirs.config_dir().to_str().unwrap());
    create_dir_all(&config_dir)?;
    let file_path = format!("{}/Settings.json", config_dir);
    fs::write(&file_path, json)?;
    Ok(format!("Particle config added to {}", file_path))
}

fn add_env_variable(matches: &ArgMatches) -> Result<String, Error> {
	let project_dirs = match ProjectDirs::from("dev", "tyrone", "littleci") {
		Some(project_dirs) => project_dirs,
		None => return Err(format_err!("Invalid $HOME path.")),
	};

	let mut persisted_config = load_app_config()?;

	let particle_name = matches.value_of("PARTICLE_NAME").unwrap();
	let particle = persisted_config.particles.get_mut(particle_name);
	match particle {
		Some(particle) => {
			let variable_name = matches.value_of("VARIABLE_NAME").unwrap().to_owned();
			let variable_value = matches.value_of("VALUE").unwrap().to_owned();
			particle.variables.insert(variable_name, variable_value);

			let json = serde_json::to_string_pretty(&persisted_config)?;
			let config_dir = String::from(project_dirs.config_dir().to_str().unwrap());
			create_dir_all(&config_dir)?;
			let file_path = format!("{}/Settings.json", config_dir);
			fs::write(&file_path, json)?;
			Ok("Particle config updated".into())
		},
		None => Err(format_err!("Particle not found: {}", particle_name)),
	}

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
                        color_line = format_args!("\x1B[{}m", colors_line.get_color(&record.level()).to_fg_str()),
                        date = chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                        target = record.target(),
                        level = colors_level.color(record.level()),
                        message = message,
                    ))
                })
                .chain(std::io::stdout())
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
			.chain(syslog::unix(syslog_formatter).unwrap())
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
        (@subcommand particle =>
            (about: "Configure particles")
            (@subcommand set =>
                (about: "Add a new particle")
                (@arg PARTICLE_NAME: +takes_value +required "Name of the particle")
                (@arg COMMAND: -c --command +takes_value +required "Command which should be executed. Note: Should include the full path to the executable if it is not in $PATH")
                (@arg WORKING_DIR: -w --working_dir +takes_value " Working directory for the command to run in.")
            )
			(@subcommand set_env =>
				(about: "Add a variable to be injected into the running job")
				(@arg PARTICLE_NAME: +takes_value +required "Name of the variable")
				(@arg VARIABLE_NAME: +takes_value +required "Name of the variable")
				(@arg VALUE: +takes_value +required "Value of the variable")
			)
        )
        (@subcommand signature =>
            (about: "Get the hashed signature to authenticate notifications")
        )
        (@subcommand serve =>
            (about: "Launch LittleCI's HTTP server")
        )
    ).get_matches();

    if let Some(matches) = command_matches.subcommand_matches("configure") {
        match generate_config(matches) {
            Ok(message) => println!("{}", message),
            Err(error) => eprintln!("Config generation failed. {}", error),
        }
    }

	if command_matches.subcommand_matches("config_path").is_some() {
		println!("{}", app_config_path());
	}

    if let Some(matches) = command_matches.subcommand_matches("particle") {
        if let Some(matches) = matches.subcommand_matches("set") {
            match add_particle_config(matches) {
                Ok(message) => println!("{}", message),
                Err(error) => eprintln!("Unable to update particle config. {}", error),
            }
        }

		if let Some(matches) = matches.subcommand_matches("set_env") {
			match add_env_variable(matches) {
				Ok(message) => println!("{}", message),
				Err(error) => eprintln!("Unable to update particle config. {}", error),
			}
		}
    }

    if command_matches.subcommand_matches("signature").is_some() {
        match get_hashed_signature() {
            Ok(signature) => println!("{}", signature),
            Err(error) => eprintln!("Unable to retrieve signature. {}", error),
        }
    }

    if command_matches.subcommand_matches("serve").is_some() {
        match load_app_config() {
            Ok(persisted_config) => {
                setup_logger(persisted_config.log_to_syslog);
                if let Err(error) = start_server(persisted_config) {
                    eprintln!("Unable to start server. {}", error);
                }
            },
            Err(_) => eprintln!("No configuration found. Please configure LittleCI first."),
        }
    }
}

