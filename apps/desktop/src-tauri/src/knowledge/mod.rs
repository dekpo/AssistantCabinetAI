//! The Knowledge Base programme's home in the client.
//!
//! Only `diagnostics` exists so far: the programme measures before it builds anything on top of
//! retrieval, so the question path and the Analyse path are instrumented first. The store, the
//! extractors and the resolver arrive in the lots that follow, each beside this file.

pub mod diagnostics;
