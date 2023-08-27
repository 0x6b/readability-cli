use serde_derive::Deserialize;

#[derive(Deserialize)]
pub struct Configuration {
    pub user_agent: String,
    pub prompts: Vec<String>,
}
