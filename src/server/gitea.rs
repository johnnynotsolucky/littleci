use std::collections::HashMap;
use serde::{self, Deserialize, Deserializer};
use serde::de::Error;
use regex::Regex;
use rocket::{Outcome, State};
use rocket::http::Status;
use rocket::request::{self, Request, FromRequest};

use crate::queue::ArbitraryData;
use crate::server::{SecretKeyError, secret_key_is_valid, AppState};

#[allow(unused_imports)]
use log::{debug, info, warn, error};
use crate::server::git::GitReference;

#[derive(Debug, Clone)]
pub struct GiteaSecret;

impl<'a, 'r> FromRequest<'a, 'r> for GiteaSecret {
    type Error = SecretKeyError;

    fn from_request(request: &'a Request<'r>) -> request::Outcome<Self, SecretKeyError> {
        let secret_key = request.headers().get("x-hub-signature").next();
        match secret_key {
            Some(secret_key) => {
                let state = request.guard::<State<AppState>>().unwrap();
                if secret_key_is_valid(&secret_key, &state) {
                    Outcome::Success(GiteaSecret)
                } else {
                    Outcome::Failure((Status::BadRequest, SecretKeyError::Invalid))
                }
            },
            _ => Outcome::Failure((Status::BadRequest, SecretKeyError::Missing))
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct GiteaPayload {
	#[serde(rename = "ref")]
	pub reference: GitReference,
	pub before: String,
	pub after: String,
}

impl From<GiteaPayload> for ArbitraryData {
	fn from(payload: GiteaPayload) -> ArbitraryData {
		let mut data: HashMap<String, String> = HashMap::new();
		data.insert("LITTLECI_GIT_BEFORE".into(), payload.before);
		data.insert("LITTLECI_GIT_AFTER".into(), payload.after);

		match payload.reference {
			GitReference::Head(branch) => data.insert("LITTLECI_GIT_BRANCH".into(), branch),
			GitReference::Tag(tag) => data.insert("LITTLECI_GIT_TAG".into(), tag),
		};
		ArbitraryData::new(data)
	}
}

