use std::sync::Arc;
use std::collections::HashMap;
use serde_json;
use serde::Serialize;
use diesel::{insert_into, update};
use diesel::prelude::*;
use diesel::sqlite::{SqliteConnection};
use chrono::{NaiveDateTime};

#[allow(unused_imports)]
use log::{debug, info, warn, error};

use schema::repositories;

use crate::config::{AppConfig, Trigger};
use crate::util::serialize_date;
use crate::{HashedValue, kebab_case};

use super::schema;

#[derive(Serialize, Debug, Clone)]
pub struct Repository {
	pub id: String,
	pub slug: String,
	pub name: String,
	pub run: String,
	pub working_dir: Option<String>,
	pub secret: String,
	pub variables: HashMap<String, String>,
	pub triggers: Vec<Trigger>,
	pub webhooks: Vec<String>,
    #[serde(serialize_with = "serialize_date")]
    pub created_at: NaiveDateTime,
    #[serde(serialize_with = "serialize_date")]
    pub updated_at: NaiveDateTime,
}

impl From<RepositoryRecord> for Repository {
	fn from(record: RepositoryRecord) -> Self {
		let variables: HashMap<String, String> = match &record.variables {
			Some(variables) => serde_json::from_str(&variables).unwrap_or_default(),
			None => HashMap::default(),
		};

		let triggers: Vec<Trigger> = match &record.triggers {
			Some(triggers) => serde_json::from_str(&triggers)
				.unwrap_or_else(|_| {
					error!("Unable to parse trigger JSON for repository {}", record.id);
					Vec::default()
				}),
			None => Vec::default(),
		};

		let webhooks: Vec<String> = match &record.webhooks {
			Some(webhooks) => serde_json::from_str(&webhooks)
				.unwrap_or_else(|_| {
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
			variables: Some(serde_json::to_string(&record.variables).expect("Unable to serialize variables to JSON".into())),
			triggers: Some(serde_json::to_string(&record.triggers).expect("Unable to serialize triggers to JSON".into())),
			webhooks: Some(serde_json::to_string(&record.webhooks).expect("Unable to serialize webhooks to JSON".into())),
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
			variables: Some(serde_json::to_string(&record.variables).expect("Unable to serialize variables to JSON".into())),
			triggers: Some(serde_json::to_string(&record.triggers).expect("Unable to serialize triggers to JSON".into())),
			webhooks: Some(serde_json::to_string(&record.webhooks).expect("Unable to serialize webhooks to JSON".into())),
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
			.expect("Unable to establish connection".into())
    }

	pub fn create(&self, repository: Repository) -> Result<Repository, String> {
        use schema::repositories::dsl::*;
        let conn = self.establish_connection();

		let repository = NewRepositoryRecord::from(repository);

		let repository_id = nanoid::custom(24, &crate::ALPHA_NUMERIC);
		let repository_secret: String = HashedValue::new(&nanoid::generate(32)).into();

        let result = insert_into(repositories)
            .values((
				&repository,
				id.eq(&repository_id),
				slug.eq(&kebab_case(&repository.name)),
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
			},
        }
	}

	pub fn save(&self, repository: Repository) -> Result<(), String> {
        use schema::repositories::dsl::*;

		let mut repository = RepositoryRecord::from(repository);

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

	pub fn all(&self) -> Vec<Repository> {
        use schema::repositories::dsl::*;

		repositories
			.load::<RepositoryRecord>(&self.establish_connection())
			.unwrap_or_else(|error| {
				error!("Error fetching repositories. {}", error);
				Vec::default()
			})
			.into_iter()
			.map(|r| Repository::from(r))
			.collect()
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
}

