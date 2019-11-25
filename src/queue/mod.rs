use std::sync::{Arc, Mutex, RwLock};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use failure::{Error, format_err};
use chrono::{NaiveDateTime, Utc};

#[allow(unused_imports)]
use log::{debug, info, warn, error};

use crate::config::AppConfig;
use crate::model::queues::Queues;
use crate::model::repositories::{Repositories, Repository};
use crate::util::serialize_date;

mod job;
use job::{JobRunner, CommandRunner};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "status", content="exit_code")]
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
    fn default() -> Self { Self::Queued }
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
    pub queues: HashMap<String, QueueService>,
}

impl QueueManager {
    pub fn new(config: Arc<AppConfig>) -> Self {
        let mut queues = HashMap::new();

		let repositories_model = Repositories::new(config.clone());

		let repositories = repositories_model.all()
			.into_iter()
			.for_each(|r| {
				queues.insert(
					r.name.clone(),
					QueueService::new(
						r.name.clone(),
						config.clone(),
						Arc::new(r.clone()),
					)
				);
			});

        Self {
            model: Arc::new(Queues::new(config.clone())),
            config,
            queues,
        }
    }

    pub fn push(&self, repository_name: &str, data: ArbitraryData) -> Result<QueueItem, Error> {
        match self.queues.get(repository_name) {
            Some(queue) => {
                let item = QueueItem::new(repository_name, data);
                self.model.push(&item);
                queue.notify();
                Ok(item)
            },
            None => Err(format_err!("Could not find queue with name {}", repository_name)),
        }
    }

    pub fn all(&self, repository: &str) -> Result<Vec<QueueItem>, Error> {
        self.model.all(repository)
    }

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
    pub queue: Arc<RwLock<Vec<QueueItem>>>,
	pub runner: Arc<dyn JobRunner>,
}

impl QueueService {
    fn new(name: String, config: Arc<AppConfig>, repository: Arc<Repository>) -> Self {
        Self {
            name: Arc::new(name),
            config,
            repository,
            processing_queue: Arc::new(Mutex::new(ProcessingQueue)),
            queue: Arc::new(RwLock::new(Vec::new())),
			runner: Arc::new(CommandRunner),
        }
    }

    fn notify(&self) {
		self.runner.process(self.clone());
    }
}
