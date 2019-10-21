#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use] extern crate diesel;

use std::sync::Arc;
use std::fs::{self, create_dir_all};
use std::process;
use std::collections::HashMap;
use std::convert::{Into, From};
use std::fmt::Write;
use sha3::{Digest, Sha3_256};
use regex::Regex;
use fern::colors::{Color, ColoredLevelConfig};
use directories::ProjectDirs;
use secstr::SecStr;
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
    get_secret,
    AppConfig,
    PersistedConfig,
    Repository,
	Trigger,
	User,
};
use crate::queue::{QueueManager, QueueService};

#[allow(unused_imports)]
use log::{debug, info, warn, error};

const DEFAULT_PORT: u16 = 8000;

#[derive(Debug, Clone)]
pub struct HashedSecret(String);

impl HashedSecret {
	pub fn new(val: &str) -> Self {
		let mut hasher = Sha3_256::new();
		hasher.input(val.as_bytes());
		let signature_bytes = hasher.result();
		let mut hashed = String::new();
		for b in signature_bytes {
			write!(&mut hashed, "{:X}", b).expect("Unable to generate secret hash");
		}
		hashed = hashed.to_lowercase();
		HashedSecret(hashed)
	}
}

impl Into<String> for HashedSecret {
	fn into(self) -> String {
		self.0
	}
}

#[derive(Debug, Clone)]
pub struct AppState {
    config: Arc<AppConfig>,
    repositories: Arc<HashMap<String, Arc<Repository>>>,
    queue_manager: Arc<QueueManager>,
    queues: Arc<HashMap<String, QueueService>>,
}

impl From<PersistedConfig> for AppState {
    fn from(configuration: PersistedConfig) -> Self {
		let secret: String = configuration.secret.clone();

        let config = AppConfig {
            secret: SecStr::from(secret),
            data_dir: configuration.data_dir,
            network_host: configuration.network_host.clone(),
            site_url: configuration.site_url.unwrap_or(configuration.network_host),
            port: configuration.port,
            log_to_syslog: configuration.log_to_syslog,
			authentication_enabled: configuration.authentication_enabled,
			users: configuration.users,
        };

        let mut repositories = HashMap::new();
        for (name, repository) in configuration.repositories.iter() {
            repositories.insert(name.to_owned(), Arc::new(repository.clone()));
        }

        let queue_manager = QueueManager::new(Arc::new(config.clone()), &repositories);

        Self {
            config: Arc::new(config),
            queue_manager: Arc::new(queue_manager),
            repositories: Arc::new(repositories),
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
        repositories: HashMap::new(),
		authentication_enabled: true,
		users: HashMap::new(),
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

fn add_repository_config(matches: &ArgMatches) -> Result<String, Error> {
    let project_dirs = match ProjectDirs::from("dev", "tyrone", "littleci") {
        Some(project_dirs) => project_dirs,
        None => return Err(format_err!("Invalid $HOME path.")),
    };

    let mut persisted_config = load_app_config()?;

	let triggers = vec![Trigger::Any];

    let repository = Repository {
        command: matches.value_of("COMMAND").unwrap().to_owned(),
        working_dir: match matches.value_of("WORKING_DIR") {
            Some(working_dir) => Some(working_dir.to_owned()),
            None => None,
        },
		triggers,
        ..Default::default()
    };

    let repository_name = kebab_case(&matches.value_of("REPOSITORY_NAME").unwrap()).to_owned();

    persisted_config.repositories.insert(repository_name, repository);

    let json = serde_json::to_string_pretty(&persisted_config)?;
    let config_dir = String::from(project_dirs.config_dir().to_str().unwrap());
    create_dir_all(&config_dir)?;
    let file_path = format!("{}/Settings.json", config_dir);
    fs::write(&file_path, json)?;
    Ok(format!("Repository config added to {}", file_path))
}

fn add_env_variable(matches: &ArgMatches) -> Result<String, Error> {
	let project_dirs = match ProjectDirs::from("dev", "tyrone", "littleci") {
		Some(project_dirs) => project_dirs,
		None => return Err(format_err!("Invalid $HOME path.")),
	};

	let mut persisted_config = load_app_config()?;

	let repository_name = matches.value_of("REPOSITORY_NAME").unwrap();
	let repository = persisted_config.repositories.get_mut(repository_name);
	match repository {
		Some(repository) => {
			let variable_name = matches.value_of("VARIABLE_NAME").unwrap().to_owned();
			let variable_value = matches.value_of("VALUE").unwrap().to_owned();
			repository.variables.insert(variable_name, variable_value);

			let json = serde_json::to_string_pretty(&persisted_config)?;
			let config_dir = String::from(project_dirs.config_dir().to_str().unwrap());
			create_dir_all(&config_dir)?;
			let file_path = format!("{}/Settings.json", config_dir);
			fs::write(&file_path, json)?;
			Ok("Repository config updated".into())
		},
		None => Err(format_err!("Repository not found: {}", repository_name)),
	}
}

fn add_user(username: &str, password: &str) -> Result<String, Error> {
	let mut persisted_config = load_app_config()?;
	let username = username.to_owned();
	let password: String = HashedSecret::new(&password).into();
	persisted_config.users.insert(username.clone(), User { username, password: password.to_owned() });

	// TODO make reusable
	let project_dirs = match ProjectDirs::from("dev", "tyrone", "littleci") {
		Some(project_dirs) => project_dirs,
		None => return Err(format_err!("Invalid $HOME path.")),
	};
	let json = serde_json::to_string_pretty(&persisted_config)?;
	let config_dir = String::from(project_dirs.config_dir().to_str().unwrap());
	create_dir_all(&config_dir)?;
	let file_path = format!("{}/Settings.json", config_dir);
	fs::write(&file_path, json)?;
	Ok("User added".into())
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
        (@subcommand repository =>
            (about: "Configure repositories")
            (@subcommand set =>
                (about: "Add a new repository")
                (@arg REPOSITORY_NAME: +takes_value +required "Name of the repository")
                (@arg COMMAND: -c --command +takes_value +required "Command which should be executed. Note: Should include the full path to the executable if it is not in $PATH")
                (@arg WORKING_DIR: -w --working_dir +takes_value " Working directory for the command to run in.")
            )
			(@subcommand set_env =>
				(about: "Add a variable to be injected into the running job")
				(@arg REPOSITORY_NAME: +takes_value +required "Name of the variable")
				(@arg VARIABLE_NAME: +takes_value +required "Name of the variable")
				(@arg VALUE: +takes_value +required "Value of the variable")
			)
        )
		(@subcommand users =>
			(about: "Manage users")
			(@subcommand add =>
				(about: "Add a new user")
				(@arg USERNAME: +takes_value +required "Username")
			)
		)
        (@subcommand secret =>
            (about: "Get the secret to authenticate notifications")
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

    if let Some(matches) = command_matches.subcommand_matches("repository") {
        if let Some(matches) = matches.subcommand_matches("set") {
            match add_repository_config(matches) {
                Ok(message) => println!("{}", message),
                Err(error) => eprintln!("Unable to update repository config. {}", error),
            }
        }

		if let Some(matches) = matches.subcommand_matches("set_env") {
			match add_env_variable(matches) {
				Ok(message) => println!("{}", message),
				Err(error) => eprintln!("Unable to update repository config. {}", error),
			}
		}
    }

    if command_matches.subcommand_matches("secret").is_some() {
        match get_secret() {
            Ok(secret) => println!("{}", secret),
            Err(error) => eprintln!("Unable to retrieve secret. {}", error),
        }
    }

    if command_matches.subcommand_matches("serve").is_some() {
        match load_app_config() {
            Ok(persisted_config) => {
                setup_logger(persisted_config.log_to_syslog).expect("Failed to initialize the logger");
                if let Err(error) = start_server(persisted_config) {
                    eprintln!("Unable to start server. {}", error);
                }
            },
            Err(_) => eprintln!("No configuration found. Please configure LittleCI first."),
        }
    }

	if let Some(matches) = command_matches.subcommand_matches("users") {
		if let Some(matches) = matches.subcommand_matches("add") {
			let password = rpassword::read_password_from_tty(Some("Password: ")).unwrap();
			let confirm = rpassword::read_password_from_tty(Some("Confirm password: ")).unwrap();
			if password != confirm {
				eprintln!("Passwords do not match.");
			} else {
				let username = matches.value_of("USERNAME").unwrap();

				match add_user(username, &password) {
					Ok(message) => println!("{}", message),
					Err(error) => eprintln!("Unable to add user: {}", error),
				}
			}
		}
	}
}

