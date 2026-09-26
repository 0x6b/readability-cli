use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    future::Future,
};

use reqwest::{Client, StatusCode, Url, header::CONTENT_TYPE};

use crate::{FetchedHtml, FetchedImage, HtmlFetcher, ImageFetcher};

#[derive(Clone, Debug)]
pub struct ReqwestFetcher {
    client: Client,
}

impl ReqwestFetcher {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[derive(Debug)]
pub enum ReqwestFetchError {
    Request(reqwest::Error),
    HttpStatus(StatusCode),
}

impl Display for ReqwestFetchError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Request(error) => Display::fmt(error, formatter),
            Self::HttpStatus(status) => write!(formatter, "HTTP error: {status}"),
        }
    }
}

impl Error for ReqwestFetchError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Request(error) => Some(error),
            Self::HttpStatus(_) => None,
        }
    }
}

impl From<reqwest::Error> for ReqwestFetchError {
    fn from(error: reqwest::Error) -> Self {
        Self::Request(error)
    }
}

impl HtmlFetcher for ReqwestFetcher {
    type Error = ReqwestFetchError;

    fn fetch_html(
        &self,
        url: &Url,
    ) -> impl Future<Output = Result<FetchedHtml, Self::Error>> + Send {
        let client = self.client.clone();
        let url = url.clone();
        async move {
            let response = client.get(url).send().await?;
            if !response.status().is_success() {
                return Err(ReqwestFetchError::HttpStatus(response.status()));
            }
            let final_url = response.url().clone();
            let html = response.text().await?;
            Ok(FetchedHtml { html, final_url })
        }
    }
}

impl ImageFetcher for ReqwestFetcher {
    fn fetch_image(
        &self,
        url: &Url,
        max_data_uri_bytes: usize,
    ) -> impl Future<Output = Option<FetchedImage>> + Send {
        let client = self.client.clone();
        let url = url.clone();
        async move {
            let mut response = client.get(url).send().await.ok()?;
            if !response.status().is_success() {
                return None;
            }
            let media_type = response
                .headers()
                .get(CONTENT_TYPE)?
                .to_str()
                .ok()?
                .split(';')
                .next()?
                .trim()
                .to_ascii_lowercase();
            if !media_type.starts_with("image/") {
                return None;
            }

            let prefix_len = format!("data:{media_type};base64,").len();
            let max_raw_bytes = max_raw_bytes(max_data_uri_bytes.checked_sub(prefix_len)?);
            if response
                .content_length()
                .is_some_and(|length| length > max_raw_bytes as u64)
            {
                return None;
            }

            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.ok()? {
                if bytes.len().checked_add(chunk.len())? > max_raw_bytes {
                    return None;
                }
                bytes.extend_from_slice(&chunk);
            }
            Some(FetchedImage { media_type, bytes })
        }
    }
}

fn max_raw_bytes(max_base64_bytes: usize) -> usize {
    if max_base64_bytes == usize::MAX {
        return usize::MAX;
    }
    max_base64_bytes / 4 * 3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_encoded_budget_to_raw_bytes() {
        assert_eq!(max_raw_bytes(3), 0);
        assert_eq!(max_raw_bytes(4), 3);
        assert_eq!(max_raw_bytes(7), 3);
        assert_eq!(max_raw_bytes(8), 6);
        assert_eq!(max_raw_bytes(usize::MAX), usize::MAX);
    }
}
