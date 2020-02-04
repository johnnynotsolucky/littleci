use chrono::{NaiveDateTime, Utc};
use diesel::prelude::*;
use diesel::{insert_into, update};
use failure::{format_err, Error};
use serde_derive::Serialize;
use serde_json;

#[allow(unused_imports)]
use log::{debug, error, info, warn};

use schema::{queue, queue_logs};

use crate::model::repositories::{Repository, RepositoryRecord};
use crate::queue::{ExecutionStatus, QueueItem, QueueLogItem};
use crate::util::serialize_date;
use crate::DbConnectionManager;

use super::schema;

#[derive(Serialize, Debug, Clone)]
pub struct JobSummary {
	id: String,
	#[serde(flatten)]
	status: ExecutionStatus,
	repository_slug: String,
	repository_name: String,
	#[serde(serialize_with = "serialize_date")]
	created_at: NaiveDateTime,
	#[serde(serialize_with = "serialize_date")]
	updated_at: NaiveDateTime,
}

impl From<(QueueRecord, RepositoryRecord)> for JobSummary {
	fn from(record: (QueueRecord, RepositoryRecord)) -> Self {
		let (job, repository) = record;
		let job = QueueItem::from((job, Vec::new()));
		let repository = Repository::from(repository);

		Self {
			id: job.id,
			status: job.status,
			repository_slug: repository.slug,
			repository_name: repository.name,
			created_at: job.created_at,
			updated_at: job.updated_at,
		}
	}
}

#[derive(Identifiable, Queryable, AsChangeset, PartialEq, Debug, Clone)]
#[table_name = "queue"]
struct QueueRecord {
	id: String,
	status: String,
	exit_code: Option<i32>,
	data: String,
	created_at: NaiveDateTime,
	updated_at: NaiveDateTime,
	repository_id: String,
}

#[derive(Identifiable, Queryable, Associations, AsChangeset, PartialEq, Debug, Clone)]
#[table_name = "queue_logs"]
#[belongs_to(QueueRecord, foreign_key = "queue_id")]
struct QueueLogRecord {
	id: i32,
	status: String,
	exit_code: Option<i32>,
	created_at: NaiveDateTime,
	queue_id: String,
}

impl From<(&str, &Option<i32>)> for ExecutionStatus {
	fn from(status: (&str, &Option<i32>)) -> ExecutionStatus {
		match status {
			("cancelled", None) => ExecutionStatus::Cancelled,
			("queued", None) => ExecutionStatus::Queued,
			("running", None) => ExecutionStatus::Running,
			("failed", Some(exit_code)) => ExecutionStatus::Failed(*exit_code),
			("completed", None) => ExecutionStatus::Completed,
			(_, _) => ExecutionStatus::Unknown,
		}
	}
}

impl Into<(String, Option<i32>)> for ExecutionStatus {
	fn into(self) -> (String, Option<i32>) {
		match self {
			ExecutionStatus::Cancelled => ("cancelled".into(), None),
			ExecutionStatus::Queued => ("queued".into(), None),
			ExecutionStatus::Running => ("running".into(), None),
			ExecutionStatus::Failed(exit_code) => ("failed".into(), Some(exit_code)),
			ExecutionStatus::Completed => ("completed".into(), None),
			ExecutionStatus::Unknown => ("unknown".into(), None),
		}
	}
}

impl From<(QueueRecord, Vec<QueueLogRecord>)> for QueueItem {
	fn from(record: (QueueRecord, Vec<QueueLogRecord>)) -> QueueItem {
		let (record, logs) = record;
		QueueItem {
			id: record.id,
			repository_id: record.repository_id,
			status: ExecutionStatus::from((&*record.status, &record.exit_code)),
			data: serde_json::from_str(&record.data).unwrap(),
			created_at: record.created_at,
			updated_at: record.updated_at,
			logs: logs.into_iter().map(QueueLogItem::from).collect(),
		}
	}
}

impl From<QueueLogRecord> for QueueLogItem {
	fn from(record: QueueLogRecord) -> QueueLogItem {
		QueueLogItem {
			status: ExecutionStatus::from((&*record.status, &record.exit_code)),
			created_at: record.created_at,
		}
	}
}

#[derive(Insertable, Debug)]
#[table_name = "queue"]
struct NewQueueRecord {
	id: String,
	status: String,
	exit_code: Option<i32>,
	data: String,
	created_at: NaiveDateTime,
	updated_at: NaiveDateTime,
	repository_id: String,
}

impl From<&QueueItem> for NewQueueRecord {
	fn from(item: &QueueItem) -> Self {
		let (status, exit_code) = item.status.clone().into();

		Self {
			id: item.id.clone(),
			status,
			exit_code,
			data: serde_json::to_string(&item.data).unwrap(),
			created_at: item.created_at,
			updated_at: item.updated_at,
			repository_id: item.repository_id.clone(),
		}
	}
}

#[derive(Insertable, Debug)]
#[table_name = "queue_logs"]
struct NewQueueLogRecord {
	status: String,
	exit_code: Option<i32>,
	created_at: NaiveDateTime,
	queue_id: String,
}

#[derive(Debug)]
pub struct Queues {
	connection_manager: DbConnectionManager,
}

impl Queues {
	pub fn new(connection_manager: DbConnectionManager) -> Self {
		Self {
			connection_manager: connection_manager.clone(),
		}
	}

	pub fn push(&self, item: &QueueItem) {
		use schema::queue::dsl::*;

		let result = insert_into(queue)
			.values(NewQueueRecord::from(item))
			.execute(&*self.connection_manager.get_write());

		// TODO Don't fail silently here, rather fail in the calling function
		match result {
			Err(error) => error!("Unable to persist queue item. {}", error),
			_ => {
				if let Err(error) = self.add_queue_log_item(&item) {
					error!("Unable to update queue log for {}. {}", item.id, error);
				}
			}
		};
	}

	pub fn next_queued(&self, record_id: &str) -> Option<QueueItem> {
		use schema::queue::dsl::*;

		let (queued_status, _) = ExecutionStatus::Queued.into();
		let record = queue
			.filter(repository_id.eq(record_id))
			.filter(status.eq(queued_status))
			.order(created_at.asc())
			.first::<QueueRecord>(&self.connection_manager.get_read());

		match record {
			Ok(record) => Some(QueueItem::from((record, Vec::new()))),
			Err(_) => None,
		}
	}

	pub fn update_status(&self, item: &QueueItem) -> Result<(), Error> {
		use schema::queue::dsl::*;

		let (new_status, new_exit_code) = item.status.clone().into();

		let result = update(queue.find(&item.id))
			.set((
				status.eq(new_status),
				exit_code.eq(new_exit_code),
				updated_at.eq(Utc::now().naive_utc()),
			))
			.execute(&*self.connection_manager.get_write());

		match result {
			Err(error) => Err(format_err!(
				"Unable to update status for {}. {}",
				item.id,
				error
			)),
			_ => match self.add_queue_log_item(&item) {
				Err(error) => Err(format_err!(
					"Unable to update status for {}. {}",
					item.id,
					error
				)),
				_ => Ok(()),
			},
		}
	}

	fn add_queue_log_item(&self, item: &QueueItem) -> Result<(), Error> {
		use schema::queue_logs::dsl::*;

		let (new_status, new_exit_code) = item.status.clone().into();

		let result = insert_into(queue_logs)
			.values(NewQueueLogRecord {
				status: new_status,
				exit_code: new_exit_code,
				created_at: Utc::now().naive_utc(),
				queue_id: item.id.clone(),
			})
			.execute(&*self.connection_manager.get_write());

		match result {
			Err(error) => Err(format_err!(
				"Unable to update status for {}. {}",
				item.id,
				error
			)),
			_ => Ok(()),
		}
	}

	pub fn all(&self) -> Result<Vec<JobSummary>, Error> {
		use schema::repositories;

		let records = queue::table
			.order(queue::dsl::created_at.desc())
			.inner_join(repositories::table)
			.limit(30)
			.load::<(QueueRecord, RepositoryRecord)>(&self.connection_manager.get_read());

		match records {
			Ok(records) => Ok(records
				.into_iter()
				.map(|record| JobSummary::from(record))
				.collect()),
			Err(error) => {
				error!("Unable to fetch jobs. {}", error);
				Err(format_err!("Unable to fetch jobs.",))
			}
		}
	}

	pub fn all_for_repository(&self, repository: &str) -> Result<Vec<QueueItem>, Error> {
		use schema::queue::dsl::*;

		let records = queue
			.filter(repository_id.eq(repository))
			.order(created_at.desc())
			.load::<QueueRecord>(&self.connection_manager.get_read());

		match records {
			Ok(records) => Ok(records
				.into_iter()
				.map(|record| QueueItem::from((record, Vec::new())))
				.collect()),
			Err(error) => {
				error!("Unable to fetch jobs for {}. {}", repository, error);
				Err(format_err!(
					"Unable to fetch jobs for {}. {}",
					repository,
					error
				))
			}
		}
	}

	pub fn job(&self, repository: &str, job_id: &str) -> Result<QueueItem, Error> {
		use schema::queue::dsl::*;

		let record = queue
			.filter(id.eq(job_id))
			.filter(repository_id.eq(repository))
			.order(created_at.desc())
			.first::<QueueRecord>(&self.connection_manager.get_read());

		match record {
			Ok(record) => {
				let logs = QueueLogRecord::belonging_to(&record)
					.load::<QueueLogRecord>(&self.connection_manager.get_read());

				let logs = match logs {
					Ok(logs) => logs,
					Err(error) => {
						error!("Unable to load job logs. {}", error);
						Vec::new()
					}
				};
				Ok(QueueItem::from((record, logs)))
			}
			Err(error) => {
				error!(
					"Unable to fetch job {} for {}. {}",
					job_id, repository, error
				);
				Err(format_err!(
					"Unable to fetch job {} for {}. {}",
					job_id,
					repository,
					error
				))
			}
		}
	}
}
