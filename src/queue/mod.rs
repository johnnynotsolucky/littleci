use std::sync::{Arc, Mutex, RwLock};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use failure::{Error, format_err};
use chrono::{NaiveDateTime, Utc};

#[allow(unused_imports)]
use log::{debug, info, warn, error};

use crate::config::{AppConfig, Particle};
use crate::model::Queue;
use crate::util::serialize_date;

mod job;
use job::{JobRunner, CommandRunner};

const ALPHA_NUMERIC: [char; 62] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c',
    'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r',
    's', 't', 'u', 'v', 'w', 'x', 'y', 'z', 'A', 'B', 'C', 'D', 'E', 'F', 'G',
    'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V',
    'W', 'X', 'Y', 'Z'
];

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

    pub particle: String,

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
    fn new(particle: &str, data: ArbitraryData) -> Self {
        Self {
            id: nanoid::custom(24, &ALPHA_NUMERIC),
            particle: particle.to_owned(),
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
    pub model: Arc<Queue>,
    pub queues: HashMap<String, QueueService>,
}

impl QueueManager {
    pub fn new(config: Arc<AppConfig>, particles: &HashMap<String, Arc<Particle>>) -> Self {
        let model = Arc::new(Queue::new(config.clone()));
        let mut queues = HashMap::new();
        for (name, particles) in particles.iter() {
            queues.insert(
                name.to_owned(),
                QueueService::new(
                    name.to_owned(),
                    config.clone(),
                    particles.clone(),
                    model.clone(),
                )
            );
        }

        Self {
            model: Arc::new(Queue::new(config.clone())),
            config,
            queues,
        }
    }

    pub fn push(&self, particle_name: &str, data: ArbitraryData) -> Result<QueueItem, Error> {
        match self.queues.get(particle_name) {
            Some(queue) => {
                let item = QueueItem::new(particle_name, data);
                self.model.push(&item);
                queue.notify();
                Ok(item)
            },
            None => Err(format_err!("Could not find queue with name {}", particle_name)),
        }
    }

    pub fn all(&self, particle: &str) -> Result<Vec<QueueItem>, Error> {
        self.model.all(particle)
    }

    pub fn job(&self, particle: &str, id: &str) -> Result<QueueItem, Error> {
        self.model.job(particle, id)
    }
}

#[derive(Debug)]
pub struct ProcessingQueue;

#[derive(Debug, Clone)]
pub struct QueueService {
    pub name: Arc<String>,
    pub config: Arc<AppConfig>,
    pub particle: Arc<Particle>,
    pub processing_queue: Arc<Mutex<ProcessingQueue>>,
    pub queue: Arc<RwLock<Vec<QueueItem>>>,
    pub model: Arc<Queue>,
	pub runner: Arc<dyn JobRunner>,
}

impl QueueService {
    fn new(name: String, config: Arc<AppConfig>, particle: Arc<Particle>, model: Arc<Queue>) -> Self {
        Self {
            name: Arc::new(name),
            config,
            particle,
            processing_queue: Arc::new(Mutex::new(ProcessingQueue)),
            queue: Arc::new(RwLock::new(Vec::new())),
            model,
			runner: Arc::new(CommandRunner),
        }
    }

    // fn add(&self, item: &QueueItem) {
    //     self.queue.write().unwrap().push(item.clone());
    //     debug!("Added item {} to queue {}", &item.id, &self.name);
    //     debug!("Queue {} size is {}", &self.name, &self.queue.read().unwrap().len());
    //     self.notify();
    // }

    fn notify(&self) {
		self.runner.process(self.clone());
    }
}
