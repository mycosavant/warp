use super::*;

fn file(path: &str, symbols: &str) -> FileContext {
    FileContext {
        path: path.to_owned(),
        symbols: symbols.to_owned(),
    }
}

/// The corpus every test below ranks against. Shaped like a real outline
/// payload: a path, then one indented `type_prefix name (line n)` per symbol,
/// which is what `FileOutline::to_string` emits.
fn corpus() -> Vec<FileContext> {
    // Ordered so that no test's expected answer is also the first entry.
    // Both of the "finds the right file" tests below passed against an
    // unranked passthrough when the target happened to lead the list, which
    // is a test that cannot fail rather than a test that passes.
    vec![
        file("docs/notes.md", "  heading Overview (line 1)"),
        file(
            "src/terminal/session.rs",
            "  fn determine_session_type (line 723)\n  enum SessionType (line 700)",
        ),
        file(
            "src/http/egress.rs",
            "  fn redirect_if_blocked (line 88)\n  const DENIED_HOSTS (line 20)",
        ),
        file(
            "src/ai/read_files.rs",
            "  fn read_local_file_context (line 12)\n  struct ReadFilesExecutor (line 40)",
        ),
    ]
}

/// The ordinary case, and the one the whole change exists to keep working.
#[test]
fn a_query_naming_a_symbol_finds_the_file_that_declares_it() {
    let ranked = rank_locally("redirect_if_blocked", &corpus(), LOCAL_RANK_LIMIT);

    assert_eq!(
        ranked.first().map(String::as_str),
        Some("src/http/egress.rs"),
        "the file whose outline holds the symbol should rank first"
    );
}

/// The claim the module doc makes about the tokenizer, tested rather than
/// asserted: `_` is a sub-token separator, so the parts of an identifier are
/// searchable without the caller knowing how it was spelled.
#[test]
fn an_underscored_identifier_is_found_by_its_parts() {
    let ranked = rank_locally("read files", &corpus(), LOCAL_RANK_LIMIT);

    assert_eq!(
        ranked.first().map(String::as_str),
        Some("src/ai/read_files.rs"),
        "`read_files` indexes as `read_files`, `read` and `files`"
    );
}

/// The honest loss, pinned so nobody reads it as a bug and nobody quietly
/// "fixes" it by returning arbitrary files.
///
/// A model-backed ranker can connect a query to a file with no shared token.
/// This cannot. Returning nothing is the correct answer for a lexical ranker
/// and is better than returning something unrelated with confidence.
#[test]
fn a_query_sharing_no_token_returns_nothing_rather_than_something_wrong() {
    let ranked = rank_locally("photosynthesis chlorophyll", &corpus(), LOCAL_RANK_LIMIT);

    assert!(
        ranked.is_empty(),
        "a lexical ranker with no lexical match has no answer, got {ranked:?}"
    );
}

/// Two runs over the same corpus rank the same way.
///
/// Tantivy returns equally-scored documents in index order, and the index is
/// built from a directory walk — so without the explicit path tie-break the
/// order is stable only for as long as the tree is.
#[test]
fn an_equal_scoring_corpus_ranks_the_same_way_twice() {
    let mut forwards = corpus();
    let backwards: Vec<FileContext> = forwards.iter().rev().cloned().collect();
    forwards.rotate_left(1);

    assert_eq!(
        rank_locally("session", &forwards, LOCAL_RANK_LIMIT),
        rank_locally("session", &backwards, LOCAL_RANK_LIMIT),
        "ranking must not depend on the order the walk produced the files"
    );
}

/// The limit is a context budget for the agent, so it is enforced rather than
/// advisory.
#[test]
fn no_more_than_the_limit_comes_back() {
    let many: Vec<FileContext> = (0..50)
        .map(|n| {
            file(
                &format!("src/session_{n}.rs"),
                "  fn session_handler (line 1)",
            )
        })
        .collect();

    assert_eq!(rank_locally("session", &many, 3).len(), 3);
    assert!(rank_locally("session", &many, 0).is_empty());
}

/// Nothing to rank is not an error, because the caller treats an empty
/// shortlist as "no relevant files" and an error as a failed turn.
#[test]
fn an_empty_corpus_or_query_is_an_empty_ranking_not_a_failure() {
    assert!(rank_locally("session", &[], LOCAL_RANK_LIMIT).is_empty());
    assert!(rank_locally("   ", &corpus(), LOCAL_RANK_LIMIT).is_empty());
}
