use jsonwebtoken::{decode, encode, Algorithm, Header, Validation};
use rocket::http::Status;
use rocket::request::{self, FromRequest, Request};
use rocket::{Outcome, State};
use serde_derive::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::config::{AppConfig, AuthenticationType};
use crate::model::users::Users;
use crate::{AppState, HashedPassword};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPayload {
	username: String,
	exp: u128,
}

pub struct AuthenticationPayload(Option<UserPayload>);

impl<'a, 'r> FromRequest<'a, 'r> for AuthenticationPayload {
	type Error = String;

	fn from_request(request: &'a Request<'r>) -> request::Outcome<AuthenticationPayload, String> {
		// check if auth type is simple
		let state = request.guard::<State<AppState>>().unwrap();
		match state.config.authentication_type {
			// Just pass through
			AuthenticationType::NoAuthentication => Outcome::Success(AuthenticationPayload(None)),
			// Validate the Bearer token
			AuthenticationType::Simple => {
				if let Some(authorization) = request.headers().get_one("authorization") {
					let parts: Vec<_> = authorization.split(" ").collect();
					if parts.len() == 2 {
						if parts[0] == "Bearer" {
							let token_data = decode::<UserPayload>(
								&parts[1],
								&state.config.secret.unsecure(),
								&Validation::new(Algorithm::HS256),
							);
							return match token_data {
								Ok(token_data) => {
									Outcome::Success(AuthenticationPayload(Some(token_data.claims)))
								}
								Err(error) => {
									Outcome::Failure((Status::Unauthorized, format!("{}", error)))
								}
							};
						}
					}
				}

				Outcome::Failure((Status::Unauthorized, "Not Authorized".into()))
			}
		}
	}
}

impl UserPayload {
	pub fn new(username: &str) -> Self {
		// TODO Should I expect something to go wrong here?
		let exp = SystemTime::now()
			.checked_add(Duration::from_secs(60))
			.unwrap()
			.duration_since(UNIX_EPOCH)
			.unwrap()
			.as_millis();

		Self {
			username: username.to_owned(),
			exp,
		}
	}

	pub fn into_token(&self, config: &AppConfig) -> String {
		let token = encode(&Header::default(), self, &config.secret.unsecure()).unwrap();
		token
	}
}

pub fn authenticate_user(
	config: Arc<AppConfig>,
	username: &str,
	password: &str,
) -> Result<UserPayload, String> {
	match config.authentication_type {
		AuthenticationType::NoAuthentication => Err("User authentication disabled".into()),
		AuthenticationType::Simple => {
			let users = Users::new(config);
			let user_record = users.find_by_username(username);
			match user_record {
				Some(user) => {
					let verified = HashedPassword::verify(&user.password, password);
					if verified {
						Ok(UserPayload::new(username))
					} else {
						Err("Passwords do not match".into())
					}
				}
				None => Err("User not found".into()),
			}
		}
	}
}
