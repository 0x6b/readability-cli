mod article;

use anyhow::Result;
use article::Article;
use article_scraper::ArticleScraper;
use html2md::parse_html;
use reqwest::{Client, Url};

pub struct Scraper {
    client: Client,
}

impl Scraper {
    pub fn try_new(user_agent: &str) -> Result<Self> {
        let client = Client::builder().user_agent(user_agent).build()?;
        Ok(Self { client })
    }

    pub async fn scrape(&self, url: &Url) -> Result<Article> {
        let article = ArticleScraper::new(None)
            .await
            .parse(url, false, &self.client, None)
            .await?;

        Ok(Article {
            url: url.clone(),
            title: article.title.unwrap_or_else(|| "No Title".into()),
            body: parse_html(&article.html.unwrap_or_default()),
        })
    }
}
