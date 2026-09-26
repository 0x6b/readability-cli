//! High-level extraction and archive rendering used by the CLI and external consumers.

mod archive;

pub use archive::{
    ArchiveDocument, GENERATOR, GENERATOR_VERSION, ImageMode, ParseImageModeError, RenderOptions,
    RenderedDocument, collect_image_sources, extract_and_render, format_utc, image_data_uri,
    render_markdown,
};
pub use rdbl_core::{ExtractOptions, ExtractResult, extract};
pub use url::Url;
