#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    JsonObject,
    JsonArray,
    Text,
}
