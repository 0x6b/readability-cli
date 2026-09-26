//! High-level extraction and archive rendering used by the CLI and external consumers.

mod archive;
#[cfg(feature = "reqwest-fetcher")]
mod image_fetch;

pub use archive::{
    ArchiveDocument, ArchiveError, FetchedImage, GENERATOR, GENERATOR_VERSION, ImageFetcher,
    ImageMode, ParseImageModeError, RenderOptions, RenderedDocument, extract_and_render,
    extract_and_render_with_fetcher, format_utc, render_markdown, render_markdown_with_fetcher,
};
#[cfg(feature = "reqwest-fetcher")]
pub use image_fetch::ReqwestImageFetcher;
pub use rdbl_core::{ExtractOptions, ExtractResult, extract};
pub use url::Url;
