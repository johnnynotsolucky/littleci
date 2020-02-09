use chrono::{NaiveDateTime, Utc};
use failure::{format_err, Error};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::{thread, time};

#[allow(unused_imports)]
use log::{debug, error, info, warn};

use crate::config::AppConfig;
use crate::model::queues::Queues;
use crate::model::repositories::Repositories;
use crate::util::serialize_date;
use crate::DbConnectionManager;

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
	pub connection_manager: DbConnectionManager,
	pub model: Arc<Queues>,
	pub queues: Arc<RwLock<HashMap<String, QueueService>>>,
}

impl QueueManager {
	pub fn new(connection_manager: DbConnectionManager, config: Arc<AppConfig>) -> Self {
		let mut queues = HashMap::new();

		// Load all repositories to restart any jobs which were waiting in the queue.
		let repositories_model = Repositories::new(connection_manager.clone());
		for r in repositories_model.all().into_iter() {
			let queue = QueueService::new(
				connection_manager.clone(),
				config.clone(),
				Arc::new(r.id.clone()),
			);
			queue.notify();
			queues.insert(r.slug, queue);
		}

		Self {
			connection_manager: connection_manager.clone(),
			config,
			model: Arc::new(Queues::new(connection_manager.clone())),
			queues: Arc::new(RwLock::new(queues)),
		}
	}

	pub fn shutdown<'a>(&self) {
		info!("Shutting down job queues.");
		for queue in self.queues.write().values_mut() {
			queue.notify_shutdown();
		}

		// TODO do this in a thread or non-blocking somehow? Use a callback so that we can notify
		// the calling function when the job count hits zero.
		loop {
			info!("Waiting for running jobs to complete.");
			let services_active: Vec<bool> = self
				.queues
				.read()
				.values()
				.map(|q| q.is_processing())
				.filter(|r| *r == true)
				.collect();

			debug!("Running jobs remaining: {}.", services_active.len());
			if services_active.len() == 0 {
				break;
			}

			thread::sleep(time::Duration::from_millis(5000));
		}
		info!("All job queues have completed.");
	}

	/// Preemptively removes the queue associated with the repository from the queue_manager.
	pub fn notify_deleted(&self, repository_id: &str) {
		let repository =
			Repositories::new(self.connection_manager.clone()).find_by_id(repository_id);
		match repository {
			Some(repository) => {
				info!("Removing queue for repository {}", &repository_id);
				let mut queues = self.queues.write();
				queues.remove(&repository.slug);
			}
			None => warn!(
				"Could not remove queue for repository {}. Not found.",
				&repository_id
			),
		}
	}

	pub fn push(&self, repository_slug: &str, data: ArbitraryData) -> Result<QueueItem, Error> {
		let repositories_model = Repositories::new(self.connection_manager.clone());

		let mut service_item: Option<(QueueService, QueueItem)> = None;
		// First see if we the service already exists in the queues map.
		{
			let queues = self.queues.read();
			if let Some(queue) = queues.get(repository_slug) {
				if let Some(repository) = repositories_model.find_by_slug(&repository_slug) {
					if !repository.deleted {
						service_item =
							Some((queue.clone(), QueueItem::new(&repository.id, data.clone())));
					} else {
						error!(
							"Repository {} has been marked as deleted. Not adding job to queue.",
							&repository.id
						);
						return Err(format_err!("Repository has been deleted."));
					}
				}
			}
		}

		// If it doesn't, create a new service for the repository
		// XXX This seems a bit tacky, but I couldn't think of another way of doing this without
		// creating a read lock and blocking a write lock if we needed to create a new service.
		if service_item.is_none() {
			match repositories_model.find_by_slug(&repository_slug) {
				Some(repository) => {
					if !repository.deleted {
						let repository_id = repository.id.clone();
						let queue = QueueService::new(
							self.connection_manager.clone(),
							self.config.clone(),
							Arc::new(repository.id.clone()),
						);

						let mut queues = self.queues.write();
						queues.insert(repository_slug.clone().into(), queue.clone());

						service_item = Some((queue, QueueItem::new(&repository_id, data)));
					} else {
						error!(
							"Repository {} has been marked as deleted. Not creating queue.",
							&repository.id
						);
						return Err(format_err!("Repository has been deleted."));
					}
				}
				None => return Err(format_err!("Could not find repository {}", repository_slug)),
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
pub enum ServiceState {
	Active,
	Inactive,
}

#[derive(Debug, Clone)]
pub struct QueueService {
	pub config: Arc<AppConfig>,
	pub connection_manager: DbConnectionManager,
	pub repository_id: Arc<String>,
	pub processing_queue: Arc<Mutex<ProcessingQueue>>,
	pub runner: Arc<dyn JobRunner>,
	pub service_state: Arc<Mutex<ServiceState>>,
}

impl QueueService {
	fn new(
		connection_manager: DbConnectionManager,
		config: Arc<AppConfig>,
		repository_id: Arc<String>,
	) -> Self {
		Self {
			config,
			connection_manager,
			repository_id,
			processing_queue: Arc::new(Mutex::new(ProcessingQueue)),
			runner: Arc::new(CommandRunner),
			service_state: Arc::new(Mutex::new(ServiceState::Active)),
		}
	}

	fn notify(&self) {
		self.runner.process(self.clone());
	}

	fn is_processing(&self) -> bool {
		self.processing_queue.try_lock().is_none()
	}

	fn notify_shutdown(&mut self) {
		let service_state = self.service_state.try_lock();
		match service_state {
			Some(mut service_state) => {
				*service_state = ServiceState::Inactive;
			}
			None => info!("Service state is transitioning."),
		}
	}
}
