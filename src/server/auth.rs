use std::time::{SystemTime, UNIX_EPOCH, Duration};
use rocket::{State, Outcome};
use rocket::request::{self, Request, FromRequest};
use rocket::http::Status;
use serde_derive::{Serialize, Deserialize};
use jsonwebtoken::{encode, decode, Header, Algorithm, Validation};

use crate::{AppState, HashedPassword};
use crate::config::{AppConfig, AuthenticationType};

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
			AuthenticationType::NoAuthentication => {
				Outcome::Success(AuthenticationPayload(None))
			},
			AuthenticationType::Simple => {
				if let Some(authorization) = request.headers().get_one("authorization") {
					let parts: Vec<_> = authorization.split(" ").collect();
					if parts.len() == 2 {
						if parts[0] == "Bearer" {
							// TODO Fetch config secret
							let token_data = decode::<UserPayload>(&parts[1], &state.config.secret.unsecure(), &Validation::new(Algorithm::HS256));
							return match token_data {
								Ok(token_data) => Outcome::Success(AuthenticationPayload(Some(token_data.claims))),
								Err(error) => Outcome::Failure((Status::Unauthorized, format!("{}", error)))
							}
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
	config: &AppConfig,
	username: &str,
	password: &str
) -> Result<UserPayload, String>
{
	match config.authentication_type {
		AuthenticationType::NoAuthentication => {
			Err("User authentication disabled".into())
		},
		AuthenticationType::Simple => {
			match config.users.get(username) {
				Some(user) => {
					let verified = HashedPassword::verify(&user, password);
					if verified {
						Ok(UserPayload::new(username))
					} else {
						Err("Passwords do not match".into())
					}
				},
				None => Err("User not found".into()),
			}
		},
	}
}
