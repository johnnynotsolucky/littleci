use chrono::{NaiveDateTime, Utc};
use failure::{format_err, Error};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

#[allow(unused_imports)]
use log::{debug, error, info, warn};

use crate::config::AppConfig;
use crate::model::queues::Queues;
use crate::model::repositories::{Repositories, Repository};
use crate::util::serialize_date;

mod job;
use job::{CommandRunner, JobRunner};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "status", content = "exit_code")]
pub enum ExecutionStatus {
	/// User terminated execution
	#[serde(rename = "cancelled")]
	Cancelled,

	#[serde(rename = "queued")]
	/// Queued for execution
	Queued,

	#[serde(rename = "running")]
	/// Execution is currently in progress
	Running,

	#[serde(rename = "failed")]
	/// Execution failed with an exit code
	Failed(i32),

	/// Execution completed successfully
	#[serde(rename = "completed")]
	Completed,

	/// Unknown status
	#[serde(rename = "unknown")]
	Unknown,
}

impl Default for ExecutionStatus {
	fn default() -> Self {
		Self::Queued
	}
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ArbitraryData(HashMap<String, String>);

impl ArbitraryData {
	pub fn new(data: HashMap<String, String>) -> Self {
		Self(data)
	}

	pub fn inner(&self) -> &HashMap<String, String> {
		&self.0
	}
}

/// Data relating to an execution.
#[derive(Serialize, Debug, Clone)]
pub struct QueueItem {
	/// A random system-generated execution identifier.
	pub id: String,

	pub repository: String,

	/// Current status of the execution
	#[serde(flatten)]
	pub status: ExecutionStatus,

	/// Any user-defined data can go here. It'll be injected into the `Command`
	/// environment when the command is executed.
	pub data: ArbitraryData,

	///
	#[serde(serialize_with = "serialize_date")]
	pub created_at: NaiveDateTime,

	///
	#[serde(serialize_with = "serialize_date")]
	pub updated_at: NaiveDateTime,

	pub logs: Vec<QueueLogItem>,
}

impl QueueItem {
	fn new(repository: &str, data: ArbitraryData) -> Self {
		Self {
			id: nanoid::custom(24, &crate::ALPHA_NUMERIC),
			repository: repository.to_owned(),
			status: ExecutionStatus::Queued,
			data,
			created_at: Utc::now().naive_utc(),
			updated_at: Utc::now().naive_utc(),
			logs: Vec::new(),
		}
	}
}

#[derive(Serialize, Debug, Clone)]
pub struct QueueLogItem {
	#[serde(flatten)]
	pub status: ExecutionStatus,

	#[serde(serialize_with = "serialize_date")]
	pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct QueueManager {
	pub config: Arc<AppConfig>,
	pub model: Arc<Queues>,
	pub queues: Arc<RwLock<HashMap<String, QueueService>>>,
}

impl QueueManager {
	pub fn new(config: Arc<AppConfig>) -> Self {
		Self {
			model: Arc::new(Queues::new(config.clone())),
			config,
			queues: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	pub fn push(&self, repository_name: &str, data: ArbitraryData) -> Result<QueueItem, Error> {
		// First see if we the service already exists in the queues map.
		let mut service_item: Option<(QueueService, QueueItem)> = match self.queues.read() {
			Ok(queues) => match queues.get(repository_name) {
				Some(queue) => Some((queue.clone(), QueueItem::new(repository_name, data.clone()))),
				None => None,
			},
			Err(error) => return Err(format_err!("Error reading from queues. {}", error)),
		};

		// If it doesn't, create a new service for the repository
		// XXX This seems a bit tacky, but I couldn't think of another way of doing this without
		// createing a read lock and blocking a write lock if we needed to create a new service.
		if service_item.is_none() {
			let repositories_model = Repositories::new(self.config.clone());

			match repositories_model.find_by_slug(&repository_name) {
				Some(repository) => {
					let queue = QueueService::new(
						repository.name.clone(),
						self.config.clone(),
						Arc::new(repository),
					);

					match self.queues.write() {
						Ok(mut queues) => {
							queues.insert(repository_name.clone().into(), queue.clone())
						}
						Err(error) => {
							return Err(format_err!("Error writing to queues. {}", error))
						}
					};

					service_item = Some((queue, QueueItem::new(repository_name, data)));
				}
				None => {
					return Err(format_err!(
						"Could not find queue with name {}",
						repository_name
					))
				}
			}
		}

		// We shouldn't get to this point without service_item being `Some()`
		let (queue, item) = service_item.expect("Unable to read from queue");
		// Add the job to the database and notify the queue service that there's something to
		// process
		self.model.push(&item);
		queue.notify();
		Ok(item)
	}

	// TODO drop this. Dependencies can just use the model directly
	pub fn all(&self, repository: &str) -> Result<Vec<QueueItem>, Error> {
		self.model.all(repository)
	}

	// TODO drop this. Dependencies can just use the model directly
	pub fn job(&self, repository: &str, id: &str) -> Result<QueueItem, Error> {
		self.model.job(repository, id)
	}
}

#[derive(Debug)]
pub struct ProcessingQueue;

#[derive(Debug, Clone)]
pub struct QueueService {
	pub name: Arc<String>,
	pub config: Arc<AppConfig>,
	pub repository: Arc<Repository>,
	pub processing_queue: Arc<Mutex<ProcessingQueue>>,
	pub runner: Arc<dyn JobRunner>,
}

impl QueueService {
	fn new(name: String, config: Arc<AppConfig>, repository: Arc<Repository>) -> Self {
		Self {
			name: Arc::new(name),
			config,
			repository,
			processing_queue: Arc::new(Mutex::new(ProcessingQueue)),
			runner: Arc::new(CommandRunner),
		}
	}

	fn notify(&self) {
		self.runner.process(self.clone());
	}
}
