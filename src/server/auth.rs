use serde_derive::{Serialize, Deserialize};
use jwt::{encode, decode, Header, Algorithm, Validation};

pub enum AuthenticationType {
	Basic,
	User,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    company: String
}
