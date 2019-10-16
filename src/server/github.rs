use std::io::Read;
use std::collections::HashMap;
use std::str;
use std::fmt::Write;
use serde::{self, Deserialize, Deserializer};
use serde::de::Error;
use regex::Regex;
use rocket::{Outcome, State, Data};
use rocket::http::Status;
use rocket::data::{self, FromDataSimple};
use rocket::request::{self, Request, FromRequest};
use sha1::Sha1;
use hmac::{Hmac, Mac};

use crate::queue::ArbitraryData;
use crate::server::{SecretKeyError, secret_key_is_valid, AppState};
use crate::server::git::GitReference;

#[allow(unused_imports)]
use log::{debug, info, warn, error};

#[derive(Deserialize, Debug, Clone)]
pub struct GitHubPayload {
	#[serde(rename = "ref")]
	pub reference: GitReference,
	pub before: String,
	pub after: String,
}

const LIMIT: u64 = 26214400; // 25MB

type HmacSha1 = Hmac<Sha1>;

impl FromDataSimple for GitHubPayload {
    type Error = SecretKeyError;

    fn from_data(request: &Request, data: Data) -> data::Outcome<Self, SecretKeyError> {
        let signature = request.headers().get("x-hub-signature").next();

		if signature.is_none() {
            return Outcome::Failure((Status::BadRequest, SecretKeyError::Missing))
		}

		let signature = signature.unwrap();
		let state = request.guard::<State<AppState>>().unwrap();

        let mut payload = Vec::new();
        if let Err(_) = data.open().take(LIMIT).read_to_end(&mut payload) {
            return Outcome::Failure((Status::BadRequest, SecretKeyError::BadData));
        }

		if let Ok(mut mac) = HmacSha1::new_varkey(state.config.secret.unsecure()) {
			mac.input(&payload);

			let signature = hex::decode(&signature).unwrap();
			if mac.verify(&signature).is_ok() {
				let payload = serde_json::from_slice(&payload);
				match payload {
					Ok(payload) => Outcome::Success(payload),
					Err(_) => Outcome::Failure((Status::BadRequest, SecretKeyError::Invalid))
				}
			} else {
				Outcome::Failure((Status::BadRequest, SecretKeyError::Invalid))
			}
		} else {
			Outcome::Failure((Status::InternalServerError, SecretKeyError::Unknown))
		}
    }
}

impl From<GitHubPayload> for ArbitraryData {
	fn from(payload: GitHubPayload) -> ArbitraryData {
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
