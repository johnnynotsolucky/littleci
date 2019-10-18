use rocket::Outcome;
use rocket::request::{self, Request, FromRequest};
use rocket::http::{Status, ContentType};
use serde_derive::{Serialize, Deserialize};
use jsonwebtoken::{encode, decode, Header, Algorithm, Validation};

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
	username: String,
}

impl<'a, 'r> FromRequest<'a, 'r> for User {
	type Error = String;

	fn from_request(request: &'a Request<'r>) -> request::Outcome<User, String> {
		if let Some(authorization) = request.headers().get_one("authorization") {
			println!("{}", authorization);
			let parts: Vec<_> = authorization.split(" ").collect();
			if parts.len() == 2 {
				if parts[0] == "Bearer" {
					// Fetch config secret
					let token_data = decode::<User>(&parts[1], "secret".as_ref(), &Validation::new(Algorithm::HS256));
					println!("{:?}", token_data);
					return match token_data {
						Ok(token_data) => {
							println!("{:?}", token_data.claims);
							Outcome::Success(token_data.claims)
						},
						Err(error) => Outcome::Failure((Status::Unauthorized, format!("{}", error)))
					}
				}
			}
		}

		Outcome::Failure((Status::Unauthorized, "Not Authorized".into()))
	}
}

impl User {
	fn into_token(&self) -> String {
		let token = encode(&Header::default(), self, "secret".as_ref()).unwrap();
		token
	}
}

