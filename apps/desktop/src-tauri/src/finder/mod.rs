//! The file finder: one deterministic engine for the left panel's search field, the chat's set sentence and
//! the entity chips. No model, no embedding, no file read (`docs/LEXICAL-RETRIEVAL-DESIGN.md` 6).

pub mod contracts;
pub mod engine;
pub mod names;
pub mod question_frame;
pub mod rank;
pub mod set_search;
pub mod typed;

pub use contracts::{
    CardEntity, Coverage, DateRange, DocumentListing, EntityFilter, Facet, Facets, FileCard,
    FileHit, FinderQuery, FinderResult, FinderScope, InventoryFile, Locator, Reason, TextSearch,
    DEFAULT_LIMIT, MAX_LIMIT,
};
pub use engine::find;
