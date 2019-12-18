use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use serde_derive::{Deserialize, Serialize};
use std::sync::Arc;

#[allow(unused_imports)]
use log::{debug, error, info, warn};

use schema::users;

use crate::config::AppConfig;
use crate::util::{serialize_date, utc_now};
use crate::HashedPassword;

use super::schema;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
	#[serde(default)]
	pub id: String,
	pub username: String,
	#[serde(default)]
	pub password: String,
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

#[derive(Identifiable, Queryable, AsChangeset, Debug, Clone)]
#[table_name = "users"]
pub struct UserRecord {
	pub id: String,
	pub username: String,
	pub password: String,
	pub created_at: NaiveDateTime,
	pub updated_at: NaiveDateTime,
}

impl From<UserRecord> for User {
	fn from(user: UserRecord) -> Self {
		Self {
			id: user.id,
			username: user.username,
			password: user.password,
			created_at: user.created_at,
			updated_at: user.updated_at,
		}
	}
}

impl From<User> for UserRecord {
	fn from(user: User) -> Self {
		Self {
			id: user.id,
			username: user.username,
			password: user.password,
			created_at: user.created_at,
			updated_at: user.updated_at,
		}
	}
}

#[derive(AsChangeset, Deserialize, Debug)]
#[table_name = "users"]
pub struct UserPassword {
	pub password: Option<String>,
}

impl UserPassword {
	pub fn as_none() -> Self {
		Self { password: None }
	}
}

#[derive(Insertable, Debug)]
#[table_name = "users"]
pub struct NewUserRecord {
	pub username: String,
	pub password: String,
}

impl From<User> for NewUserRecord {
	fn from(user: User) -> Self {
		Self {
			username: user.username,
			password: user.password,
		}
	}
}

#[derive(Debug)]
pub struct Users {
	config: Arc<AppConfig>,
}

impl Users {
	pub fn new(config: Arc<AppConfig>) -> Self {
		Self {
			config: config.clone(),
		}
	}

	fn establish_connection(&self) -> SqliteConnection {
		SqliteConnection::establish(&format!("{}/littleci.sqlite3", self.config.data_dir))
			.expect("Unable to establish connection")
	}

	pub fn create(&self, user: User) -> Result<User, String> {
		use schema::users::dsl::*;

		if self.find_by_username(&user.username).is_some() {
			return Err(format!("Username already exists"));
		}

		let conn = self.establish_connection();

		let user_id = nanoid::custom(24, &crate::ALPHA_NUMERIC);
		let mut user_record = NewUserRecord::from(user);

		let salt = nanoid::custom(16, &nanoid::alphabet::SAFE);
		user_record.password = HashedPassword::new(&user_record.password, &salt).into();

		let result = diesel::insert_into(users)
			.values((id.eq(&user_id), user_record))
			.execute(&conn);

		// TODO Don't fail silently here, rather fail in the calling function
		match result {
			Err(error) => Err(format!("Unable to save new user. {}", error)),
			_ => match users.filter(id.eq(user_id)).first::<UserRecord>(&conn) {
				Ok(record) => Ok(User::from(record)),
				Err(error) => Err(format!("Unable to fetch saved user. {}", error)),
			},
		}
	}

	pub fn save(&self, user: User) -> Result<User, String> {
		use schema::users::dsl::*;

		let conn = self.establish_connection();

		let user = UserRecord::from(user);

		let result = diesel::update(users.filter(id.eq(&user.id)))
			.set((&user, UserPassword::as_none()))
			.execute(&conn);

		match result {
			Err(error) => Err(format!("Unable to save user. {}", error)),
			_ => match users.filter(id.eq(user.id)).first::<UserRecord>(&conn) {
				Ok(record) => Ok(User::from(record)),
				Err(error) => Err(format!("Unable to fetch saved user. {}", error)),
			},
		}
	}

	pub fn set_password(
		&self,
		user_username: &str,
		mut user_password: UserPassword,
	) -> Result<(), String> {
		use schema::users::dsl::*;

		if user_password.password.is_some() {
			let conn = self.establish_connection();

			let salt = nanoid::custom(16, &nanoid::alphabet::SAFE);
			user_password.password =
				Some(HashedPassword::new(&user_password.password.unwrap(), &salt).into());

			let result = diesel::update(users.filter(username.eq(&user_username)))
				.set(user_password)
				.execute(&conn);

			match result {
				Err(error) => Err(format!("Unable to save user. {}", error)),
				_ => Ok(()),
			}
		} else {
			Err("Password not set".into())
		}
	}

	pub fn all(&self) -> Vec<User> {
		use schema::users::dsl::*;

		users
			.load::<UserRecord>(&self.establish_connection())
			.unwrap_or_else(|error| {
				error!("Error fetching users. {}", error);
				Vec::default()
			})
			.into_iter()
			.map(|r| User::from(r))
			.collect()
	}

	pub fn delete_by_id(&self, user_id: &str) -> Result<(), String> {
		use schema::users::dsl::*;

		let result =
			diesel::delete(users.filter(id.eq(&user_id))).execute(&self.establish_connection());

		match result {
			Err(error) => Err(format!("Unable to delete user. {}", error)),
			_ => Ok(()),
		}
	}

	pub fn find_by_username(&self, user_name: &str) -> Option<User> {
		use schema::users::dsl::*;

		let record = users
			.filter(username.eq(user_name))
			.first::<UserRecord>(&self.establish_connection());

		match record {
			Ok(record) => Some(User::from(record)),
			Err(_) => None,
		}
	}
}
