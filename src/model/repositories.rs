use chrono::{NaiveDateTime, Utc};
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use serde_derive::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::sync::Arc;

#[allow(unused_imports)]
use log::{debug, error, info, warn};

use schema::repositories;

use crate::config::{AppConfig, Trigger};
use crate::util::serialize_date;
use crate::{kebab_case, HashedValue};

use super::schema;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Repository {
	#[serde(default)]
	pub id: String,
	#[serde(skip_deserializing)]
	pub slug: String,
	pub name: String,
	#[serde(default)]
	pub run: String,
	#[serde(default)]
	pub working_dir: Option<String>,
	#[serde(skip_deserializing)]
	pub secret: String,
	#[serde(default)]
	pub variables: HashMap<String, String>,
	#[serde(default)]
	pub triggers: Vec<Trigger>,
	#[serde(default)]
	pub webhooks: Vec<String>,
	#[serde(skip)]
	pub deleted: i32,
	#[serde(
		skip_deserializing,
		default = "utc_now",
		serialize_with = "serialize_date"
	)]
	pub created_at: NaiveDateTime,
	#[serde(
		skip_deserializing,
		default = "utc_now",
		serialize_with = "serialize_date"
	)]
	pub updated_at: NaiveDateTime,
}

fn utc_now() -> NaiveDateTime {
	Utc::now().naive_utc()
}

impl From<RepositoryRecord> for Repository {
	fn from(record: RepositoryRecord) -> Self {
		let variables: HashMap<String, String> = match &record.variables {
			Some(variables) => serde_json::from_str(&variables).unwrap_or_default(),
			None => HashMap::default(),
		};

		let triggers: Vec<Trigger> = match &record.triggers {
			Some(triggers) => serde_json::from_str(&triggers).unwrap_or_else(|_| {
				error!("Unable to parse trigger JSON for repository {}", record.id);
				Vec::default()
			}),
			None => Vec::default(),
		};

		let webhooks: Vec<String> = match &record.webhooks {
			Some(webhooks) => serde_json::from_str(&webhooks).unwrap_or_else(|_| {
				error!("Unable to parse webhook JSON for repository {}", record.id);
				Vec::default()
			}),
			None => Vec::default(),
		};

		Self {
			id: record.id,
			slug: record.slug,
			name: record.name,
			run: record.run,
			working_dir: record.working_dir,
			secret: record.secret,
			variables,
			triggers,
			webhooks,
			deleted: record.deleted,
			created_at: record.created_at,
			updated_at: record.updated_at,
		}
	}
}

#[derive(Identifiable, Queryable, AsChangeset, Debug, Clone)]
#[table_name = "repositories"]
pub struct RepositoryRecord {
	pub id: String,
	pub slug: String,
	pub name: String,
	pub run: String,
	pub working_dir: Option<String>,
	pub secret: String,

	/// I'm just going to store JSON in here for now
	pub variables: Option<String>,

	/// I'm just going to store JSON in here for now
	pub triggers: Option<String>,
	pub webhooks: Option<String>,
	pub deleted: i32,
	pub created_at: NaiveDateTime,
	pub updated_at: NaiveDateTime,
}

impl From<Repository> for RepositoryRecord {
	fn from(record: Repository) -> Self {
		Self {
			id: record.id,
			slug: record.slug,
			name: record.name,
			run: record.run,
			working_dir: record.working_dir,
			secret: record.secret,
			variables: Some(
				serde_json::to_string(&record.variables)
					.expect("Unable to serialize variables to JSON".into()),
			),
			triggers: Some(
				serde_json::to_string(&record.triggers)
					.expect("Unable to serialize triggers to JSON".into()),
			),
			webhooks: Some(
				serde_json::to_string(&record.webhooks)
					.expect("Unable to serialize webhooks to JSON".into()),
			),
			deleted: record.deleted,
			created_at: record.created_at,
			updated_at: record.updated_at,
		}
	}
}

#[derive(Insertable, Debug)]
#[table_name = "repositories"]
pub struct NewRepositoryRecord {
	pub name: String,
	pub run: Option<String>,
	pub working_dir: Option<String>,
	pub variables: Option<String>,
	pub triggers: Option<String>,
	pub webhooks: Option<String>,
}

impl From<Repository> for NewRepositoryRecord {
	fn from(record: Repository) -> Self {
		Self {
			name: record.name,
			run: Some(record.run),
			working_dir: record.working_dir,
			variables: Some(
				serde_json::to_string(&record.variables)
					.expect("Unable to serialize variables to JSON".into()),
			),
			triggers: Some(
				serde_json::to_string(&record.triggers)
					.expect("Unable to serialize triggers to JSON".into()),
			),
			webhooks: Some(
				serde_json::to_string(&record.webhooks)
					.expect("Unable to serialize webhooks to JSON".into()),
			),
		}
	}
}

#[derive(AsChangeset, Debug)]
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
	config: Arc<AppConfig>,
}

impl Repositories {
	pub fn new(config: Arc<AppConfig>) -> Self {
		Self {
			config: config.clone(),
		}
	}

	fn establish_connection(&self) -> SqliteConnection {
		SqliteConnection::establish(&format!("{}/littleci.sqlite3", self.config.data_dir))
			.expect("Unable to establish connection".into())
	}

	pub fn create(&self, repository: Repository) -> Result<Repository, String> {
		use schema::repositories::dsl::*;

		let repository_slug = kebab_case(&repository.name);
		if self.find_by_slug(&repository_slug).is_some() {
			return Err(format!("Repository slug already exists"));
		}

		let conn = self.establish_connection();

		let repository = NewRepositoryRecord::from(repository);

		let repository_id = nanoid::custom(24, &crate::ALPHA_NUMERIC);
		let repository_secret: String = HashedValue::new(&nanoid::generate(32)).into();

		let result = diesel::insert_into(repositories)
			.values((
				&repository,
				id.eq(&repository_id),
				slug.eq(&repository_slug),
				secret.eq(&repository_secret),
			))
			.execute(&conn);

		match result {
			Err(error) => Err(format!("Unable to save new repository. {}", error)),
			_ => {
				match repositories
					.filter(id.eq(repository_id))
					.first::<RepositoryRecord>(&conn)
				{
					Ok(record) => Ok(Repository::from(record)),
					Err(error) => Err(format!("Unable to fetch saved repository. {}", error)),
				}
			}
		}
	}

	pub fn save(&self, repository: Repository) -> Result<Repository, String> {
		use schema::repositories::dsl::*;

		let repository_slug = kebab_case(&repository.name);
		if let Some(existing_repository) = self.find_by_slug(&repository_slug) {
			if &existing_repository.id != &repository.id {
				return Err(format!("Repository slug already exists"));
			}
		}

		let conn = self.establish_connection();

		let mut repository = RepositoryRecord::from(repository);

		repository.slug = kebab_case(&repository.name);

		let result = diesel::update(repositories.filter(id.eq(&repository.id)))
			.set((
				&repository,
				slug.eq(&kebab_case(&repository.name)),
				RepositorySecret::as_none()
			))
			.execute(&conn);

		match result {
			Err(error) => Err(format!("Unable to save repository. {}", error)),
			_ => {
				match repositories
					.filter(id.eq(repository.id))
					.first::<RepositoryRecord>(&conn)
				{
					Ok(record) => Ok(Repository::from(record)),
					Err(error) => Err(format!("Unable to fetch saved repository. {}", error)),
				}
			}
		}
	}

	pub fn all(&self) -> Vec<Repository> {
		use schema::repositories::dsl::*;

		repositories
			.filter(deleted.eq(0))
			.load::<RepositoryRecord>(&self.establish_connection())
			.unwrap_or_else(|error| {
				error!("Error fetching repositories. {}", error);
				Vec::default()
			})
			.into_iter()
			.map(|r| Repository::from(r))
			.collect()
	}

	pub fn find_by_id(&self, repository_id: &str) -> Option<Repository> {
		use schema::repositories::dsl::*;

		let record = repositories
			.filter(id.eq(repository_id))
			.first::<RepositoryRecord>(&self.establish_connection());

		match record {
			Ok(record) => Some(Repository::from(record)),
			Err(_) => None,
		}
	}

	pub fn find_by_slug(&self, repository_slug: &str) -> Option<Repository> {
		use schema::repositories::dsl::*;

		let record = repositories
			.filter(slug.eq(repository_slug))
			.first::<RepositoryRecord>(&self.establish_connection());

		match record {
			Ok(record) => Some(Repository::from(record)),
			Err(_) => None,
		}
	}

	pub fn delete_by_id(&self, repository_id: &str) -> Result<(), String> {
		use schema::repositories::dsl::*;

		let result = diesel::update(repositories.filter(id.eq(&repository_id)))
			.set(deleted.eq(1))
			.execute(&self.establish_connection());

		match result {
			Err(error) => Err(format!("Unable to save repository. {}", error)),
			_ => {
				Ok(())
			}
		}
	}
}
