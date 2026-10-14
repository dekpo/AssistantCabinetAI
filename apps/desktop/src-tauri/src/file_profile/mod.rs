//! The file profile: what indexing knows about one file as a whole, built from the file itself.
//!
//! `docs/LEXICAL-RETRIEVAL-DESIGN.md` section 5. Each part is a pure function over bytes or text with
//! no database, so it is tested on generated files; the tables that store the result are a later
//! step. **A field is filled with its source and its confidence, or it is empty: never a guess.**
//!
//! The spreadsheet engine is not touched by anything here (design 4.8 and 8): the profile reads what
//! the Analyse already stored and writes only to its own tables.

pub mod build;
pub mod cells;
pub mod dates;
pub mod header;
pub mod identifiers;
pub mod metadata;
pub mod name;
pub mod store;
pub mod types;
pub mod user_types;
pub mod vocabulary;
