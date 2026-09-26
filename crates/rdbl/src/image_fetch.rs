use std::future::Future;

use reqwest::{Client, Url, header::CONTENT_TYPE};

use crate::{FetchedImage, ImageFetcher};

#[derive(Clone, Debug)]
pub struct ReqwestImageFetcher {
    client: Client,
}

impl ReqwestImageFetcher {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

impl ImageFetcher for ReqwestImageFetcher {
    fn fetch(
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
