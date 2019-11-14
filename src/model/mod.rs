use std::sync::Arc;
use serde_json;
use diesel::{insert_into, update};
use diesel::prelude::*;
use diesel::sqlite::{SqliteConnection};
use chrono::{NaiveDateTime, Utc};
use failure::{Error, format_err};

#[allow(unused_imports)]
use log::{debug, info, warn, error};

mod schema;
use schema::{users, repositories, queue, queue_logs};

use crate::config::{AppConfig};
use crate::queue::{QueueItem, QueueLogItem, ExecutionStatus};
use crate::{HashedPassword, HashedValue, kebab_case};

#[derive(Identifiable, Queryable, AsChangeset, Debug, Clone)]
#[table_name = "users"]
pub struct UserRecord {
    pub id: String,
	pub username: String,
	pub password: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable, Debug)]
#[table_name = "users"]
pub struct NewUserRecord {
	pub username: String,
	pub password: String,
}

#[derive(Debug)]
pub struct Users {
    config: Arc<AppConfig>
}

impl Users {
    pub fn new(config: Arc<AppConfig>) -> Self {
        Self { config: config.clone() }
    }

    fn establish_connection(&self) -> SqliteConnection {
        SqliteConnection::establish(
				&format!("{}/littleci.sqlite3", self.config.data_dir)
			)
			.expect("Unable to establish connection")
    }

	pub fn create(&self, mut user: NewUserRecord) -> Result<UserRecord, String> {
        use schema::users::dsl::*;
        let conn = self.establish_connection();

		let user_id = nanoid::custom(24, &crate::ALPHA_NUMERIC);

		let salt = nanoid::custom(16, &nanoid::alphabet::SAFE);
		user.password = HashedPassword::new(&user.password, &salt).into();

        let result = insert_into(users)
            .values((id.eq(&user_id), user))
            .execute(&conn);

		// TODO Don't fail silently here, rather fail in the calling function
        match result {
            Err(error) => Err(format!("Unable to save new user. {}", error)),
            _ => {
				match users
					.filter(id.eq(user_id))
					.first::<UserRecord>(&conn)
				{
					Ok(record) => Ok(record),
					Err(error) => Err(format!("Unable to fetch saved user. {}", error)),
				}
			},
        }
	}

    pub fn find_by_username(&self, user_name: &str) -> Option<UserRecord> {
        use schema::users::dsl::*;

		let record = users
			.filter(username.eq(user_name))
			.first::<UserRecord>(&self.establish_connection());

		match record {
			Ok(record) => Some(record),
			Err(_) => None,
		}
    }
}

#[derive(Identifiable, Queryable, AsChangeset, Debug, Clone)]
#[table_name = "repositories"]
pub struct RepositoryRecord {
    pub id: String,
	pub slug: String,
	pub name: String,
	pub run: Option<String>,
	pub working_dir: Option<String>,
	pub secret: String,

	/// I'm just going to store JSON in here for now
	pub variables: Option<String>,

	/// I'm just going to store JSON in here for now
	pub triggers: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable, Debug)]
#[table_name = "repositories"]
pub struct NewRepositoryRecord {
	pub name: String,
	pub run: Option<String>,
	pub working_dir: Option<String>,
	pub variables: Option<String>,
	pub triggers: Option<String>,
}

#[derive(AsChangeset, Debug, Clone)]
#[table_name = "repositories"]
pub struct RepositorySecret {
	pub secret: Option<String>,
}

impl RepositorySecret {
	pub fn as_none() -> Self {
		Self { secret: None }
	}
}

pub struct Repositories {
    config: Arc<AppConfig>
}

impl Repositories {
    pub fn new(config: Arc<AppConfig>) -> Self {
        Self { config: config.clone() }
    }

    fn establish_connection(&self) -> SqliteConnection {
        SqliteConnection::establish(
				&format!("{}/littleci.sqlite3", self.config.data_dir)
			)
			.expect("Unable to establish connection")
    }

	pub fn create(&self, repository: NewRepositoryRecord) -> Result<RepositoryRecord, String> {
        use schema::repositories::dsl::*;
        let conn = self.establish_connection();

		let repository_id = nanoid::custom(24, &crate::ALPHA_NUMERIC);
		let repository_secret: String = HashedValue::new(&nanoid::generate(32)).into();

        let result = insert_into(repositories)
            .values((
				id.eq(&repository_id),
				slug.eq(&kebab_case(&repository.name)),
				secret.eq(&repository_secret),
				repository
			))
            .execute(&conn);

        match result {
            Err(error) => Err(format!("Unable to save new repository. {}", error)),
            _ => {
				match repositories
					.filter(id.eq(repository_id))
					.first::<RepositoryRecord>(&conn)
				{
					Ok(record) => Ok(record),
					Err(error) => Err(format!("Unable to fetch saved repository. {}", error)),
				}
			},
        }
	}

	pub fn save(&self, mut repository: RepositoryRecord) -> Result<(), String> {
        use schema::repositories::dsl::*;

		repository.slug = kebab_case(&repository.name);

        let result = diesel::update(repositories)
            .set((
				repository,
				RepositorySecret::as_none(),
			))
            .execute(&self.establish_connection());

        match result {
            Err(error) => Err(format!("Unable to save repository. {}", error)),
            _ => {
				Ok(())
			},
        }
	}

	pub fn all(&self) -> Vec<RepositoryRecord> {
        use schema::repositories::dsl::*;

		repositories
			.load::<RepositoryRecord>(&self.establish_connection())
			.unwrap_or_else(|error| {
				error!("Error fetching repositories. {}", error);
				Vec::default()
			})
	}

	pub fn find_by_slug(&self, repository_slug: &str) -> Option<RepositoryRecord> {
        use schema::repositories::dsl::*;

		let record = repositories
			.filter(slug.eq(repository_slug))
			.first::<RepositoryRecord>(&self.establish_connection());

		match record {
			Ok(record) => Some(record),
			Err(_) => None,
		}
	}
}

#[derive(Identifiable, Queryable, AsChangeset, PartialEq, Debug, Clone)]
#[table_name = "queue"]
struct QueueRecord {
    id: String,
    repository: String,
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
            repository: record.repository,
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
    repository: String,
    status: String,
    exit_code: Option<i32>,
    data: String,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
}

impl From<&QueueItem> for NewQueueRecord {
    fn from(item: &QueueItem) -> Self {
        let (status, exit_code) = item.status.clone().into();

        Self {
            id: item.id.clone(),
            repository: item.repository.clone(),
            status,
            exit_code,
            data: serde_json::to_string(&item.data).unwrap(),
            created_at: item.created_at,
            updated_at: item.updated_at,
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
pub struct Queue {
    config: Arc<AppConfig>
}

impl Queue {
    pub fn new(config: Arc<AppConfig>) -> Self {
        Self { config: config.clone() }
    }

    fn establish_connection(&self) -> SqliteConnection {
        SqliteConnection::establish(
				&format!("{}/littleci.sqlite3", self.config.data_dir)
			)
			.expect("Unable to establish connection")
    }

    pub fn push(&self, item: &QueueItem) {
        use schema::queue::dsl::*;
        let conn = self.establish_connection();

        let result = insert_into(queue)
            .values(NewQueueRecord::from(item))
            .execute(&conn);

		// TODO Don't fail silently here, rather fail in the calling function
        match result {
            Err(error) => error!("Unable to persist queue item. {}", error),
            _ => {
				if let Err(error) = self.add_queue_log_item(&item) {
					error!("Unable to update queue log for {}. {}", item.id, error);
				}
			},
        };
    }

    pub fn next_queued(&self) -> Option<QueueItem> {
        use schema::queue::dsl::*;

        let (queued_status, _) = ExecutionStatus::Queued.into();
        let record = queue
            .filter(status.eq(queued_status))
            .order(created_at.desc())
            .first::<QueueRecord>(&self.establish_connection());

        match record {
            Ok(record) => {
                Some(QueueItem::from((record, Vec::new())))
            },
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
            .execute(&self.establish_connection());

        match result {
            Err(error) => Err(format_err!("Unable to update status for {}. {}", item.id, error)),
            _ => {
				match self.add_queue_log_item(&item) {
					Err(error) => Err(format_err!("Unable to update status for {}. {}", item.id, error)),
					_ => Ok(())
				}
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
            .execute(&self.establish_connection());

        match result {
            Err(error) => Err(format_err!("Unable to update status for {}. {}", item.id, error)),
            _ => Ok(())
        }
	}

    pub fn all(&self, repository_name: &str) -> Result<Vec<QueueItem>, Error> {
        use schema::queue::dsl::*;

        let records = queue
            .filter(repository.eq(repository_name))
            .order(created_at.desc())
            .load::<QueueRecord>(&self.establish_connection());

        match records {
            Ok(records) => Ok(records
				.into_iter()
				.map(|record| QueueItem::from((record, Vec::new())))
				.collect()),
            Err(error) => {
                error!("Unable to fetch jobs for {}. {}", repository_name, error);
                Err(format_err!("Unable to fetch jobs for {}. {}", repository_name, error))
            }
        }
    }

    pub fn job(&self, repository_name: &str, job_id: &str) -> Result<QueueItem, Error> {
        use schema::queue::dsl::*;

		let conn = &self.establish_connection();

        let record = queue
            .filter(id.eq(job_id))
            .filter(repository.eq(repository_name))
            .order(created_at.desc())
            .first::<QueueRecord>(conn);

        match record {
            Ok(record) => {
				let logs = QueueLogRecord::belonging_to(&record)
					.load::<QueueLogRecord>(conn);

				let logs = match logs {
					Ok(logs) => logs,
					Err(error) => {
						error!("Unable to load job logs. {}", error);
						Vec::new()
					}
				};
				Ok(QueueItem::from((record, logs)))
			},
            Err(error) => {
                error!("Unable to fetch job {} for {}. {}", job_id, repository_name, error);
                Err(format_err!("Unable to fetch job {} for {}. {}", job_id, repository_name, error))
            }
        }
    }
}

