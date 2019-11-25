use std::sync::Arc;
use serde_json;
use diesel::{insert_into, update};
use diesel::prelude::*;
use diesel::sqlite::{SqliteConnection};
use chrono::{NaiveDateTime, Utc};
use failure::{Error, format_err};

#[allow(unused_imports)]
use log::{debug, info, warn, error};

use schema::{users, repositories, queue, queue_logs};

use crate::config::{AppConfig};
use crate::queue::{QueueItem, QueueLogItem, ExecutionStatus};
use crate::{HashedPassword, HashedValue, kebab_case};

use super::schema;

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



