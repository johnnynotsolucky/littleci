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

#[derive(Debug, Clone)]
pub enum GitReference {
	Head(String),
	Tag(String),
	// TODO Do we need more ref types?
}

#[derive(Debug, Clone)]
pub struct GitHubSecret;

impl<'a, 'r> FromRequest<'a, 'r> for GitHubSecret {
    type Error = SecretKeyError;

    fn from_request(request: &'a Request<'r>) -> request::Outcome<Self, SecretKeyError> {
        let secret_key = request.headers().get("x-hub-signature").next();
        match secret_key {
            Some(secret_key) => {
                let state = request.guard::<State<AppState>>().unwrap();
                if secret_key_is_valid(&secret_key, &state) {
                    Outcome::Success(GitHubSecret)
                } else {
                    Outcome::Failure((Status::BadRequest, SecretKeyError::Invalid))
                }
            },
            _ => Outcome::Failure((Status::BadRequest, SecretKeyError::Missing))
        }
    }
}

impl<'de> Deserialize<'de> for GitReference {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		let full_ref = String::deserialize(deserializer)?;

		// TODO This could probably be neater?
		let heads_regex = Regex::new(r"^refs/heads/(.+)").unwrap();
		let reference = if let Some(captures) = heads_regex.captures(&full_ref) {
			match captures.get(1) {
				Some(capture) => Some(GitReference::Head(capture.as_str().to_owned())),
				None => None,
			}
		} else {
			let tags_regex = Regex::new(r"^refs/tags/(.+)").unwrap();
			if let Some(captures) = tags_regex.captures(&full_ref) {
				match captures.get(1) {
					Some(capture) => Some(GitReference::Tag(capture.as_str().to_owned())),
					None => None
				}
			} else {
				None
			}
		};

		// TODO probably make sure the ref is valid and not some random shit

		match reference {
			Some(reference) => Ok(reference),
			None => Err(Error::custom("Invalid ref")),
		}
	}
}

#[derive(Deserialize, Debug, Clone)]
pub struct GitHubPayload {
	#[serde(rename = "ref")]
	pub reference: GitReference,
	pub before: String,
	pub after: String,
	pub head_commit: Option<String>,
}

impl From<GitHubPayload> for ArbitraryData {
	fn from(payload: GitHubPayload) -> ArbitraryData {
		let mut data: HashMap<String, String> = HashMap::new();
		data.insert("LITTLECI_GIT_BEFORE".into(), payload.before);
		data.insert("LITTLECI_GIT_AFTER".into(), payload.after);

		if let Some(head_commit) = payload.head_commit {
			data.insert("LITTLECI_GIT_HEAD_COMMIT".into(), head_commit);
		}

		match payload.reference {
			GitReference::Head(branch) => data.insert("LITTLECI_GIT_BRANCH".into(), branch),
			GitReference::Tag(tag) => data.insert("LITTLECI_GIT_TAG".into(), tag),
		};
		ArbitraryData::new(data)
	}
}
