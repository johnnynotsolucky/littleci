use serde::{self, Deserialize, Deserializer};
use serde::de::Error;
use regex::Regex;

#[derive(Debug, Clone)]
pub enum GitReference {
	Head(String),
	Tag(String),
	// TODO Do we need more ref types?
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
