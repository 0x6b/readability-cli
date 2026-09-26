//! High-level extraction and archive rendering used by the CLI and external consumers.

mod archive;
#[cfg(feature = "reqwest-fetcher")]
mod fetch;

pub use archive::{
    ArchiveDocument, ArchiveError, FetchedHtml, FetchedImage, GENERATOR, GENERATOR_VERSION,
    HtmlFetcher, ImageFetcher, ImageMode, ParseImageModeError, RenderOptions, RenderedDocument,
    extract_and_render, extract_and_render_with_fetcher, format_utc, render_markdown,
    render_markdown_with_fetcher,
};
#[cfg(feature = "reqwest-fetcher")]
pub use fetch::{ReqwestFetchError, ReqwestFetcher};
pub use rdbl_core::{ExtractOptions, ExtractResult, extract};
pub use url::Url;
