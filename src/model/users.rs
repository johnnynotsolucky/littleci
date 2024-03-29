use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{self, Deserialize, Deserializer, Serialize};

#[allow(unused_imports)]
use log::{debug, error, info, warn};

use schema::users;

use crate::util::{serialize_date, utc_now};
use crate::DbConnectionManager;
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
pub struct UpdateUserPassword {
	#[serde(deserialize_with = "deserialize_password")]
	pub password: Option<String>,
}

fn deserialize_password<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
	D: Deserializer<'de>,
{
	let password = String::deserialize(deserializer)?;

	// TODO Something something password rules
	if password.len() > 0 {
		Ok(Some(password))
	} else {
		Ok(None)
	}
}

#[derive(Identifiable, Queryable, AsChangeset, Debug)]
#[table_name = "users"]
pub struct UpdateUserRecord {
	pub id: String,
	pub created_at: NaiveDateTime,
	pub updated_at: NaiveDateTime,
}

impl From<User> for UpdateUserRecord {
	fn from(user: User) -> Self {
		Self {
			id: user.id,
			created_at: user.created_at,
			updated_at: user.updated_at,
		}
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
	connection_manager: DbConnectionManager,
}

impl Users {
	pub fn new(connection_manager: DbConnectionManager) -> Self {
		Self { connection_manager }
	}

	pub fn create(&self, user: User) -> Result<User, String> {
		use schema::users::dsl::*;

		if self.find_by_username(&user.username).is_some() {
			return Err(format!("Username already exists"));
		}

		let user_id = nanoid::custom(24, &crate::ALPHA_NUMERIC);
		let mut user_record = NewUserRecord::from(user);

		let salt = nanoid::custom(16, &nanoid::alphabet::SAFE);
		user_record.password = HashedPassword::new(&user_record.password, &salt).into();

		let result = diesel::insert_into(users)
			.values((id.eq(&user_id), user_record))
			.execute(&*self.connection_manager.get_write());

		// TODO Don't fail silently here, rather fail in the calling function
		match result {
			Err(error) => Err(format!("Unable to save new user. {}", error)),
			_ => match users
				.filter(id.eq(user_id))
				.first::<UserRecord>(&self.connection_manager.get_read())
			{
				Ok(record) => Ok(User::from(record)),
				Err(error) => Err(format!("Unable to fetch saved user. {}", error)),
			},
		}
	}

	pub fn save(&self, user: User) -> Result<User, String> {
		use schema::users::dsl::*;

		let user = UpdateUserRecord::from(user);

		let result = diesel::update(users.filter(id.eq(&user.id)))
			.set(&user)
			.execute(&*self.connection_manager.get_write());

		match result {
			Err(error) => Err(format!("Unable to save user. {}", error)),
			_ => match users
				.filter(id.eq(user.id))
				.first::<UserRecord>(&self.connection_manager.get_read())
			{
				Ok(record) => Ok(User::from(record)),
				Err(error) => Err(format!("Unable to fetch saved user. {}", error)),
			},
		}
	}

	pub fn set_password(
		&self,
		user_username: &str,
		mut user_password: UpdateUserPassword,
	) -> Result<(), String> {
		use schema::users::dsl::*;

		match user_password.password {
			Some(new_password) => {
				let salt = nanoid::custom(16, &nanoid::alphabet::SAFE);
				user_password.password = Some(HashedPassword::new(&new_password, &salt).into());

				let result = diesel::update(users.filter(username.eq(&user_username)))
					.set(user_password)
					.execute(&*self.connection_manager.get_write());

				match result {
					Err(error) => Err(format!("Unable to save user. {}", error)),
					_ => Ok(()),
				}
			}
			None => Err("Password not set".into()),
		}
	}

	pub fn all(&self) -> Vec<User> {
		use schema::users::dsl::*;

		users
			.load::<UserRecord>(&self.connection_manager.get_read())
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

		let result = diesel::delete(users.filter(id.eq(&user_id)))
			.execute(&*self.connection_manager.get_write());

		match result {
			Err(error) => Err(format!("Unable to delete user. {}", error)),
			_ => Ok(()),
		}
	}

	pub fn find_by_id(&self, user_id: &str) -> Option<User> {
		use schema::users::dsl::*;

		let record = users
			.filter(id.eq(user_id))
			.first::<UserRecord>(&self.connection_manager.get_read());

		match record {
			Ok(record) => Some(User::from(record)),
			Err(_) => None,
		}
	}

	pub fn find_by_username(&self, user_name: &str) -> Option<User> {
		use schema::users::dsl::*;

		let record = users
			.filter(username.eq(user_name))
			.first::<UserRecord>(&self.connection_manager.get_read());

		match record {
			Ok(record) => Some(User::from(record)),
			Err(_) => None,
		}
	}
}
