use chrono::{NaiveDateTime, Utc};
use failure::{format_err, Error};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[allow(unused_imports)]
use log::{debug, error, info, warn};

use crate::config::AppConfig;
use crate::model::queues::Queues;
use crate::model::repositories::Repositories;
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

	pub repository_id: String,

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
	fn new(repository_id: &str, data: ArbitraryData) -> Self {
		Self {
			id: nanoid::custom(24, &crate::ALPHA_NUMERIC),
			repository_id: repository_id.to_owned(),
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
		// TODO What happens to jobs that were running but the service was killed before they could
		// finish?

		let mut queues = HashMap::new();

		// Load all repositories to restart any jobs which were waiting in the queue.
		let repositories_model = Repositories::new(config.clone());
		for r in repositories_model.all().into_iter() {
			let queue = QueueService::new(
				r.name.clone(),
				config.clone(),
				Arc::new(r.id.clone()),
			);
			queue.notify();
			queues.insert(r.slug, queue);
		}

		Self {
			model: Arc::new(Queues::new(config.clone())),
			config,
			queues: Arc::new(RwLock::new(queues)),
		}
	}

	pub fn push(&self, repository_slug: &str, data: ArbitraryData) -> Result<QueueItem, Error> {
		let repositories_model = Repositories::new(self.config.clone());

		let mut service_item: Option<(QueueService, QueueItem)> = None;
		// First see if we the service already exists in the queues map.
		{
			let queues = self.queues.read();
			if let Some(queue) = queues.get(repository_slug) {
				if let Some(repository) = repositories_model.find_by_slug(&repository_slug) {
					service_item =
						Some((queue.clone(), QueueItem::new(&repository.id, data.clone())));
				}
			}
		}

		// If it doesn't, create a new service for the repository
		// XXX This seems a bit tacky, but I couldn't think of another way of doing this without
		// creating a read lock and blocking a write lock if we needed to create a new service.
		if service_item.is_none() {
			match repositories_model.find_by_slug(&repository_slug) {
				Some(repository) => {
					let repository_id = repository.id.clone();
					let queue = QueueService::new(
						repository.name.clone(),
						self.config.clone(),
						Arc::new(repository.id.clone()),
					);

					let mut queues = self.queues.write();
					queues.insert(repository_slug.clone().into(), queue.clone());

					service_item = Some((queue, QueueItem::new(&repository_id, data)));
				}
				None => {
					return Err(format_err!(
						"Could not find queue with name {}",
						repository_slug
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
}

#[derive(Debug)]
pub struct ProcessingQueue;

#[derive(Debug, Clone)]
pub struct QueueService {
	pub name: Arc<String>,
	pub config: Arc<AppConfig>,
	pub repository_id: Arc<String>,
	pub processing_queue: Arc<Mutex<ProcessingQueue>>,
	pub runner: Arc<dyn JobRunner>,
}

impl QueueService {
	fn new(name: String, config: Arc<AppConfig>, repository_id: Arc<String>) -> Self {
		Self {
			name: Arc::new(name),
			config,
			repository_id,
			processing_queue: Arc::new(Mutex::new(ProcessingQueue)),
			runner: Arc::new(CommandRunner),
		}
	}

	fn notify(&self) {
		self.runner.process(self.clone());
	}
}
