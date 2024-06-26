use serde::Deserialize;

#[derive(Deserialize)]
pub struct Configuration {
    pub user_agent: String,
}

impl Default for Configuration {
    fn default() -> Self {
        Self { user_agent: "rdbl/0.1.0".to_string() }
    }
}
