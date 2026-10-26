//! Native reader core. Engine state is confined to the document worker.
pub mod document;
mod epub_compat;
mod fb2_adapter;
pub mod fonts;
mod kindle_adapter;
pub mod paging;
pub mod reading;
pub mod rendering;
pub mod security;
pub mod settings;
pub mod storage;
mod text_adapter;
pub mod zoom;
