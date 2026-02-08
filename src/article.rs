use std::fmt;

use reqwest::Url;

pub struct Article {
    pub url: Url,
    pub title: String,
    pub body: String,
}

impl fmt::Display for Article {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "# {}\n\n{}", self.title, self.body)
    }
}
