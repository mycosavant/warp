//! Ranking candidate files against a query without leaving the machine.
//!
//! `get_relevant_files` has two paths to a shortlist. The embedding path is
//! gone in this fork — `FullSourceCodeEmbedding` is in `fork::FORCE_DISABLED`
//! because `generate_embeddings` uploads source. The path that remains builds
//! tree-sitter outlines locally and then, with two or more candidates, `POST`s
//! `/ai/relevant_files` with every file's path, its symbol names and the
//! comments attached to them, receiving a list of paths back.
//!
//! That second call is what this module replaces. Building the outline is local
//! and searching it was not, which is a distinction nobody would guess from the
//! feature's name.
//!
//! **Nothing here is new machinery.** `warp_search_core` is already an `app`
//! dependency — it is the tantivy-backed searcher behind the command palette —
//! and it gives BM25 scoring, field weights, and the same `CustomTokenizer` the
//! rest of the app searches with. So a file at `src/ai/read_files.rs` indexes
//! as `src`, `ai`, `read_files`, `read` and `files`, and the query "read files"
//! finds it. Term queries are also unioned with a penalised edit-distance-1
//! fuzzy query, so a typo still lands.
//!
//! **What is honestly lost, rather than papered over.** This is lexical. The
//! server-side ranker is a model and can match "how do I authenticate" to
//! `login.rs` with no shared token; this cannot, and returns nothing rather
//! than something wrong. The tokenizer splits on `_`, `-`, `/`, `\` and `:` but
//! **not** on camelCase, so `readFiles` stays one token and only matches a
//! query naming it that way. Both are the cost of the boundary and are stated
//! here so the next person does not read an empty result as a bug.

use std::cmp::Ordering;

use warp_search_core::define_search_schema;
use warp_search_core::searcher::DEFAULT_MEMORY_BUDGET;

use super::api::FileContext;

define_search_schema!(
    schema_name: RELEVANT_FILES_SCHEMA,
    config_name: RelevantFilesConfig,
    search_doc: RelevantFileDoc,
    identifying_doc: RelevantFileId,
    search_fields: [path: 2.0, symbols: 1.0],
    id_fields: [ordinal: u64]
);

/// How many paths a local ranking returns.
///
/// Each one becomes a whole-file context entry for the agent, so this is a
/// context budget rather than a search parameter. Ten is chosen to sit between
/// the two behaviours already in this file: below
/// `MINIMUM_FILE_COUNT_FOR_API_CALL` the caller returns *every* candidate, and
/// the server path returns whatever the model picked.
pub(crate) const LOCAL_RANK_LIMIT: usize = 10;

/// Ranks `files` against `query`, best first, and returns at most `limit`
/// paths.
///
/// Returns the paths exactly as they arrived — relative to the repository root
/// — because that is what the caller joins onto the root and what the server
/// path returned.
///
/// Every failure returns an empty ranking rather than propagating. A search
/// index that will not build is a reason to have no shortlist, not a reason to
/// fail the agent's turn, and the caller already treats an empty set as "no
/// relevant files".
pub(crate) fn rank_locally(query: &str, files: &[FileContext], limit: usize) -> Vec<String> {
    if files.is_empty() || limit == 0 || query.trim().is_empty() {
        return Vec::new();
    }

    let searcher = RELEVANT_FILES_SCHEMA.create_searcher(DEFAULT_MEMORY_BUDGET);
    if let Err(err) =
        searcher.build_index(
            files
                .iter()
                .enumerate()
                .map(|(ordinal, file)| RelevantFileDoc {
                    path: file.path.clone(),
                    symbols: file.symbols.clone(),
                    ordinal: ordinal as u64,
                }),
        )
    {
        log::warn!(
            "fork: could not index {} files for ranking: {err}",
            files.len()
        );
        return Vec::new();
    }

    let mut matches = match searcher.search_full_doc(query) {
        Ok(matches) => matches,
        Err(err) => {
            log::warn!("fork: local ranking failed for {query:?}: {err}");
            return Vec::new();
        }
    };

    // Ties broken by path so a repository ranks the same way twice. Tantivy
    // returns documents in index order within a score, and the index is built
    // from a directory walk, so without this the order is stable only for as
    // long as the tree is.
    matches.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.values.path.cmp(&b.values.path))
    });

    matches
        .into_iter()
        .take(limit)
        .map(|matched| matched.values.path)
        .collect()
}

#[cfg(test)]
#[path = "local_rank_tests.rs"]
mod tests;
