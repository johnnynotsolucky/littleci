use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "swagger/dist/"]
pub struct ApiDefinitionUi;

#[derive(RustEmbed)]
#[folder = "static/"]
pub struct StaticAssets;

