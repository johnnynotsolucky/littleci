use rocket::http::{ContentType, Status};
use rocket::request::Request;
use rocket::response::{Responder, Response};
use rust_embed::RustEmbed;
use std::io::Cursor;
use std::path::PathBuf;

#[derive(RustEmbed)]
#[folder = "swagger/dist/"]
pub struct ApiDefinitionUi;

#[derive(RustEmbed)]
#[folder = "static/"]
pub struct StaticAssets;

#[derive(RustEmbed)]
#[folder = "ui/dist/"]
pub struct UI;

pub enum AssetType {
	ApiDefinitionUi,
	StaticAssets,
	UI,
}

pub struct Assets {
	pub file_path: PathBuf,
	pub asset_type: AssetType,
}

impl Responder<'_> for Assets {
	fn respond_to(self, req: &Request) -> Result<Response<'static>, Status> {
		let get_fn = match self.asset_type {
			AssetType::ApiDefinitionUi => ApiDefinitionUi::get,
			AssetType::StaticAssets => StaticAssets::get,
			AssetType::UI => UI::get,
		};

		if let Some(asset) = get_fn(self.file_path.to_str().unwrap()) {
			let data = asset.as_ref().to_vec();

			let stream_response = rocket::response::Stream::chunked(Cursor::new(data), 10);
			let mut response = stream_response.respond_to(&req)?;

			if let Some(extension) = self.file_path.extension() {
				if let Some(content_type) =
					ContentType::from_extension(&extension.to_string_lossy())
				{
					response.set_header(content_type);
				}
			}

			Ok(response)
		} else {
			Err(Status::NotFound)
		}
	}
}
