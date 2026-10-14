//! The name port of the set sentence: when X is the name of exactly one entity of the knowledge base, the
//! answer is the files linked to that entity as well as the files that write its surfaces
//! (`docs/LEXICAL-RETRIEVAL-DESIGN.md` 7).
//!
//! The router receives nothing from the knowledge base; the command that runs the finder holds the
//! connection and gives it here. Two entities, a possible match or no entity at all leave X a phrase: a
//! name is never chosen between two people, and an unknown name is searched as the words it is.

use rusqlite::Connection;

use crate::knowledge::normalize::fold;
use crate::knowledge::phonetic::PhoneticEncoder;
use crate::knowledge::resolve::{
    DeterministicResolver, EntityResolver, Resolution, ResolveContext, ResolveMode, StoreLookup,
    SurfaceInput,
};
use crate::knowledge::targeting::QueryVocabulary;
use crate::knowledge::{store, AliasKind, EntityStatus};

/// The most surfaces searched as phrases besides what the user typed.
const MAX_SURFACES: usize = 6;

/// The one entity a name stands for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedEntity {
    pub entity_id: i64,
    /// The entity's canonical name, shown on screen only.
    pub display: String,
    /// The surfaces the files write it under, canonical name first, to be searched as phrases. A surname
    /// alone or a title with a surname is not one: several people can carry it.
    pub surfaces: Vec<String>,
}

/// Resolves what the user typed to exactly one entity, or to nothing.
pub trait NameResolver {
    fn resolve_one(&self, text: &str) -> Option<NamedEntity>;
}

/// A resolver that knows nobody: every X stays a phrase.
pub struct NoNames;

impl NameResolver for NoNames {
    fn resolve_one(&self, _text: &str) -> Option<NamedEntity> {
        None
    }
}

/// The resolver over the knowledge base's tables, in the question mode of the deterministic resolver.
pub struct KnowledgeNames<'a> {
    pub connection: &'a Connection,
    pub vocabulary: &'a QueryVocabulary,
}

impl NameResolver for KnowledgeNames<'_> {
    fn resolve_one(&self, text: &str) -> Option<NamedEntity> {
        let typed = text.trim();
        if typed.is_empty() {
            return None;
        }
        let lookup = StoreLookup::new(self.connection, &self.vocabulary.titles);
        let encoder: &dyn PhoneticEncoder = self.vocabulary.encoder.as_ref();
        let context = ResolveContext {
            mode: ResolveMode::Query,
            titles: &self.vocabulary.titles,
            encoder,
            lookup: &lookup,
            selection: None,
            source_entities: None,
        };
        let resolution = DeterministicResolver.resolve(&SurfaceInput::new(typed), &context);
        let Resolution::Existing { entity, .. } = resolution else {
            return None;
        };
        let record = store::entity_with_aliases(self.connection, entity.entity_id)
            .ok()
            .flatten()?;
        if record.entity.status != EntityStatus::Active {
            return None;
        }
        let mut surfaces: Vec<String> = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        let candidates = std::iter::once(record.entity.canonical_name.clone()).chain(
            record
                .aliases
                .iter()
                .filter(|alias| {
                    !alias.kind.is_ambiguous_by_nature()
                        && !matches!(alias.kind, AliasKind::Initial | AliasKind::Abbreviation)
                })
                .map(|alias| alias.display.clone()),
        );
        for surface in candidates {
            let folded = fold(&surface);
            // A lone word or an initial is not a name to search for: it would meet every namesake.
            if folded.split_whitespace().count() < 2 || seen.contains(&folded) {
                continue;
            }
            seen.push(folded);
            surfaces.push(surface);
            if surfaces.len() >= MAX_SURFACES {
                break;
            }
        }
        Some(NamedEntity {
            entity_id: record.entity.entity_id,
            display: record.entity.canonical_name,
            surfaces,
        })
    }
}
