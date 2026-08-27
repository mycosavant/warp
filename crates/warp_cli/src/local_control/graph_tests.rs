//! The half of a graph run that can be decided without spending a token.
//!
//! Everything here is the *scheduler*: which nodes may start, what prompt they
//! start with, and which plans should never have been started at all. The
//! spawning itself is one `agent.spawn` per node and is verified by running it
//! (`.fork/TASKS.md`, "T7.1 — as built").

use super::*;

fn plan(text: &str) -> Plan {
    toml::from_str(text).expect("the fixture should parse")
}

fn states(pairs: &[(&str, NodeState)]) -> HashMap<String, NodeState> {
    pairs
        .iter()
        .map(|(id, state)| ((*id).to_owned(), state.clone()))
        .collect()
}

fn done(output: &str) -> NodeState {
    NodeState::Done {
        conversation_id: "c-1".to_owned(),
        output: output.to_owned(),
    }
}

const SURVEY_AND_FIX: &str = r#"
[defaults]
allow_tools = ["read-only"]

[[node]]
id = "survey"
prompt = "List every file under src/ that still calls the old API."

[[node]]
id = "fix"
prompt = "Migrate those files to the new API."
allow_tools = ["read-only", "APPLY_FILE_DIFFS"]
needs = [{ node = "survey", pass = "the list of files" }]
"#;

/// The example in the module documentation, parsed.
///
/// Doc examples that were never run are how a format acquires a spelling
/// nobody can use.
#[test]
fn the_documented_plan_parses() {
    let plan = plan(SURVEY_AND_FIX);

    assert!(validate(&plan).is_ok());
    assert_eq!(plan.nodes.len(), 2);
    assert_eq!(
        plan.defaults.allow_tools.as_deref(),
        Some(&["read-only".to_owned()][..])
    );
    assert_eq!(plan.nodes[1].needs.len(), 1);
    assert_eq!(plan.nodes[1].needs[0].node(), "survey");
    assert_eq!(plan.nodes[1].needs[0].pass(), Some("the list of files"));
}

/// A bare id is an edge with no payload, not a second kind of edge.
#[test]
fn a_bare_id_is_an_ordering_edge() {
    let plan = plan(
        r#"
        [[node]]
        id = "a"
        prompt = "first"

        [[node]]
        id = "b"
        prompt = "second"
        needs = ["a"]
        "#,
    );

    assert!(validate(&plan).is_ok());
    assert_eq!(plan.nodes[1].needs[0].node(), "a");
    assert_eq!(
        plan.nodes[1].needs[0].pass(),
        None,
        "ordering without a payload hands nothing along"
    );
}

/// A node's own allowlist wins; otherwise it inherits.
///
/// The inheritance is the point of `[defaults]`: a plan whose every node is
/// read-only except one should say `read-only` once, because a plan that
/// repeats the restriction on every line is a plan where one line will
/// eventually be missing it.
#[test]
fn a_node_inherits_the_default_allowlist_unless_it_says_otherwise() {
    let plan = plan(SURVEY_AND_FIX);
    let states = states(&[("survey", done("src/a.rs\nsrc/b.rs"))]);

    let survey = spawn_params(&plan, &plan.nodes[0], &states, None, None);
    assert_eq!(
        survey.allow_tools.as_deref(),
        Some(&["read-only".to_owned()][..])
    );

    let fix = spawn_params(
        &plan,
        &plan.nodes[1],
        &states,
        Some("parent".to_owned()),
        None,
    );
    assert_eq!(
        fix.allow_tools,
        Some(vec!["read-only".to_owned(), "APPLY_FILE_DIFFS".to_owned()]),
        "a node that names its own tools is not also given the defaults"
    );
    assert_eq!(fix.parent_conversation_id.as_deref(), Some("parent"));
    assert_eq!(
        fix.name.as_deref(),
        Some("fix"),
        "the id is the name by default"
    );
}

/// The handoff, which is the half of an edge that is not ordering.
#[test]
fn an_edge_with_a_payload_appends_the_upstream_answer() {
    let plan = plan(SURVEY_AND_FIX);
    let states = states(&[("survey", done("src/a.rs\nsrc/b.rs"))]);

    let prompt = compose_prompt(&plan.nodes[1], &states, None);
    assert!(prompt.starts_with("Migrate those files to the new API."));
    assert!(
        prompt.contains("--- From `survey` (the list of files):"),
        "the payload needs a label, or it is a wall of text under no heading: {prompt}"
    );
    assert!(prompt.contains("src/a.rs\nsrc/b.rs"));
}

#[test]
fn an_ordering_edge_hands_nothing_along() {
    let plan = plan(
        r#"
        [[node]]
        id = "a"
        prompt = "first"

        [[node]]
        id = "b"
        prompt = "second"
        needs = ["a"]
        "#,
    );
    let states = states(&[("a", done("the whole answer"))]);

    assert_eq!(compose_prompt(&plan.nodes[1], &states, None), "second");
}

/// Nothing starts until everything it needs has finished.
#[test]
fn a_node_waits_for_every_edge() {
    let plan = plan(SURVEY_AND_FIX);

    let nothing = states(&[]);
    assert_eq!(
        ready(&plan, &nothing, 0, 4)
            .iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>(),
        vec!["survey"],
        "`fix` needs `survey`"
    );

    let running = states(&[(
        "survey",
        NodeState::Running {
            conversation_id: "c-1".to_owned(),
        },
    )]);
    assert!(
        ready(&plan, &running, 1, 4).is_empty(),
        "a running dependency is not a finished one"
    );

    let finished = states(&[("survey", done("src/a.rs"))]);
    assert_eq!(
        ready(&plan, &finished, 0, 4)
            .iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>(),
        vec!["fix"]
    );
}

/// `--max-parallel` bounds load, not correctness.
#[test]
fn no_more_than_max_parallel_start_at_once() {
    let plan = plan(
        r#"
        [[node]]
        id = "a"
        prompt = "a"
        [[node]]
        id = "b"
        prompt = "b"
        [[node]]
        id = "c"
        prompt = "c"
        "#,
    );

    assert_eq!(ready(&plan, &states(&[]), 0, 2).len(), 2);
    assert_eq!(ready(&plan, &states(&[]), 1, 2).len(), 1);
    assert_eq!(
        ready(&plan, &states(&[]), 2, 2).len(),
        0,
        "the cap counts what is already running"
    );
}

/// A failure stops what depended on it, and says which thing stopped it.
///
/// Skipped is not failed. A run that reports six failures when one node failed
/// and five were waiting on it has buried the only fact worth acting on.
#[test]
fn a_failure_blocks_its_dependents_and_names_the_blocker() {
    let plan = plan(SURVEY_AND_FIX);
    let failed = states(&[(
        "survey",
        NodeState::Failed {
            conversation_id: "c-1".to_owned(),
            reason: "the conversation ended `error`".to_owned(),
        },
    )]);

    assert_eq!(
        newly_blocked(&plan, &failed),
        vec![("fix".to_owned(), "survey".to_owned())]
    );
    assert!(
        ready(&plan, &failed, 0, 4).is_empty(),
        "a blocked node must not also look ready"
    );
}

/// A branch with nothing wrong with it keeps running.
#[test]
fn a_failure_on_one_branch_leaves_the_others_alone() {
    let plan = plan(
        r#"
        [[node]]
        id = "a"
        prompt = "a"
        [[node]]
        id = "b"
        prompt = "b"
        [[node]]
        id = "after-a"
        prompt = "after a"
        needs = ["a"]
        "#,
    );
    let failed = states(&[(
        "a",
        NodeState::Failed {
            conversation_id: "c-1".to_owned(),
            reason: "nope".to_owned(),
        },
    )]);

    assert_eq!(
        newly_blocked(&plan, &failed),
        vec![("after-a".to_owned(), "a".to_owned())]
    );
    assert_eq!(
        ready(&plan, &failed, 0, 4)
            .iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>(),
        vec!["b"],
        "`b` never depended on `a`"
    );
}

/// A cycle is caught before anything spawns, and its members are named.
///
/// At runtime a cycle is invisible: it presents as a scheduler that stops
/// finding work, which reads like a hang. By then there are children running
/// and a partial result to reason about.
#[test]
fn a_cycle_is_refused_with_the_nodes_in_it() {
    let plan = plan(
        r#"
        [[node]]
        id = "a"
        prompt = "a"
        needs = ["c"]
        [[node]]
        id = "b"
        prompt = "b"
        needs = ["a"]
        [[node]]
        id = "c"
        prompt = "c"
        needs = ["b"]
        "#,
    );

    let error = validate(&plan).expect_err("a, b and c wait on each other");
    assert_eq!(error.code, ErrorCode::InvalidParams);
    assert!(
        error.message.contains("a -> b -> c"),
        "the message should name the cycle: {}",
        error.message
    );
}

#[test]
fn the_plans_that_cannot_run_are_refused_one_reason_at_a_time() {
    let unknown = plan(
        r#"
        [[node]]
        id = "a"
        prompt = "a"
        needs = ["nope"]
        "#,
    );
    assert!(
        validate(&unknown)
            .expect_err("`nope` is not a node")
            .message
            .contains("not in this plan")
    );

    let duplicate = plan(
        r#"
        [[node]]
        id = "a"
        prompt = "one"
        [[node]]
        id = "a"
        prompt = "two"
        "#,
    );
    assert!(
        validate(&duplicate)
            .expect_err("two nodes named `a`")
            .message
            .contains("share the id")
    );

    let itself = plan(
        r#"
        [[node]]
        id = "a"
        prompt = "a"
        needs = ["a"]
        "#,
    );
    assert!(
        validate(&itself)
            .expect_err("`a` waits for `a`")
            .message
            .contains("needs itself")
    );

    let empty: Plan = toml::from_str("").expect("an empty file is valid TOML");
    assert!(
        validate(&empty)
            .expect_err("a plan with no nodes")
            .message
            .contains("no `[[node]]`")
    );
}

/// A typo in a plan is refused rather than ignored.
///
/// `deny_unknown_fields` matters more here than in most places: the fields are
/// the guardrails. `allow_tool = ["read-only"]` silently accepted is a node
/// that runs with no restriction at all, discovered by reading what it did.
#[test]
fn a_misspelled_field_is_refused() {
    let error = toml::from_str::<Plan>(
        r#"
        [[node]]
        id = "a"
        prompt = "a"
        allow_tool = ["read-only"]
        "#,
    )
    .expect_err("`allow_tool` is not `allow_tools`");
    assert!(
        error.to_string().contains("allow_tool"),
        "the error should name the field: {error}"
    );
}

/// The order a reader wants: what runs together, and in how many rounds.
#[test]
fn check_reports_the_plan_in_waves() {
    let plan = plan(
        r#"
        [[node]]
        id = "b"
        prompt = "b"
        needs = ["a"]
        [[node]]
        id = "a"
        prompt = "a"
        [[node]]
        id = "c"
        prompt = "c"
        [[node]]
        id = "d"
        prompt = "d"
        needs = ["b", "c"]
        "#,
    );

    assert_eq!(
        waves(&plan),
        vec![
            vec!["a".to_owned(), "c".to_owned()],
            vec!["b".to_owned()],
            vec!["d".to_owned()],
        ]
    );
}

/// The race a poller falls into, closed.
///
/// A conversation polled in the instant after `agent.spawn` is not busy *yet*.
/// Reading that as "finished" gives the next node a handoff that is an empty
/// string, and the graph runs to completion having done nothing.
#[test]
fn a_conversation_that_has_not_started_is_not_a_finished_one() {
    assert!(!turn_is_finished(false, None), "spawned, no exchange yet");
    assert!(
        !turn_is_finished(false, Some(false)),
        "an exchange, unfinished"
    );
    assert!(
        !turn_is_finished(true, Some(true)),
        "busy again on the next turn"
    );
    assert!(turn_is_finished(false, Some(true)));
}

/// A multi-line answer still reads as one line in the progress output.
#[test]
fn progress_shows_enough_of_an_answer_to_recognize_it() {
    assert_eq!(first_line("  one\ntwo\nthree  "), "one");
    assert_eq!(first_line(""), "");
    // By character, not by byte: `String` slicing inside a glyph panics, and a
    // model's answer is exactly where one turns up.
    let wide = "🌍".repeat(100);
    assert!(first_line(&wide).ends_with('…'));
    assert_eq!(first_line(&wide).chars().count(), 73);
}

/// The printed format is a plan that would actually run.
///
/// The claim `graph schema` makes about itself, asserted rather than trusted.
/// Documentation drifts silently; this cannot, because renaming a field
/// without editing the schema fails here.
#[test]
fn the_schema_is_a_valid_plan() {
    let plan: Plan = toml::from_str(SCHEMA).expect("the schema should parse as a plan");
    validate(&plan).expect("the schema should be a runnable plan");

    assert_eq!(
        waves(&plan),
        vec![
            vec!["survey".to_owned()],
            vec!["fix".to_owned()],
            vec!["report".to_owned()],
            vec!["review".to_owned()],
        ],
        "the example should demonstrate edges, not four independent nodes — and \
         the reviewer has to come last, which is the fence proving itself"
    );
}

/// The schema shows both spellings of an edge, because the shorthand is the
/// one an agent will reach for and the payload is the one that matters.
#[test]
fn the_schema_documents_both_kinds_of_edge() {
    assert!(SCHEMA.contains(r#"needs = ["survey"]"#));
    assert!(SCHEMA.contains(r#"pass = "the files""#));
    assert!(
        SCHEMA.contains("allow_tools"),
        "the guardrail is the field most worth spelling correctly"
    );
}

// ---------------------------------------------------------------------------
// The run record and the sealed-plan guard (`.fork/TASKS.md`, T13.1).
//
// These are Tusk's `test_supersede_chain.py` cases under the fork's shape:
// rewrite-on-edit, can't-retire-cleared, can't-retire-sealed. What is missing
// from that list is deliberate — there is no `supersede`/`cancel` operation
// here, because the plan is a file and the edit primitive is a text editor.
// The guard therefore reads the edit after the fact instead of validating a
// patch before it, and the two rules below are what that reading amounts to.
// ---------------------------------------------------------------------------

/// A three-node chain, so a reach-back can be shown travelling *through* work
/// that is itself finished.
const CHAIN: &str = r#"
[[node]]
id = "a"
prompt = "a"

[[node]]
id = "b"
prompt = "b"
needs = ["a"]

[[node]]
id = "c"
prompt = "c"
needs = ["b"]
"#;

/// The states half of an [`Outcome`], for the many tests that assert nothing.
fn outcome(states: HashMap<String, NodeState>) -> Outcome {
    Outcome {
        states,
        verdicts: HashMap::new(),
    }
}

fn all_done(plan: &Plan) -> HashMap<String, NodeState> {
    plan.nodes
        .iter()
        .map(|node| (node.id.clone(), done(&format!("{} finished", node.id))))
        .collect()
}

fn nodes_in(violations: &[Violation]) -> Vec<&str> {
    violations
        .iter()
        .map(|violation| match violation {
            Violation::Edited { node, .. } | Violation::ReachedBack { node, .. } => node.as_str(),
        })
        .collect()
}

/// The record says what it is, so a file found without context can be read.
#[test]
fn a_record_round_trips_and_names_its_own_format() {
    let plan = plan(SURVEY_AND_FIX);
    let record = build_record(&plan, &outcome(all_done(&plan)));

    let text = serde_json::to_string(&record).expect("a record should serialize");
    let parsed: RunRecord = serde_json::from_str(&text).expect("and parse back");

    assert_eq!(parsed.record, RECORD_KIND);
    assert_eq!(parsed.version, RECORD_VERSION);
    assert_eq!(parsed.nodes.len(), 2);
    assert_eq!(
        parsed.nodes["survey"].settled,
        done("survey finished"),
        "a resume hands the recorded answer downstream, so it has to survive the file"
    );
    assert_eq!(parsed.nodes["survey"].fingerprint.len(), 64);
}

/// A node that never settled is not in the record, because there is nothing to
/// say about it that the plan does not already say.
#[test]
fn only_settled_nodes_are_recorded() {
    let plan = plan(SURVEY_AND_FIX);
    let record = build_record(&plan, &outcome(states(&[("survey", done("one"))])));

    assert_eq!(record.nodes.keys().collect::<Vec<_>>(), vec!["survey"]);
}

/// The ordinary case: nothing was touched, so nothing is wrong.
#[test]
fn an_untouched_plan_agrees_with_its_own_record() {
    let plan = plan(SURVEY_AND_FIX);
    let record = build_record(&plan, &outcome(all_done(&plan)));

    assert_eq!(sealed(&plan, &record), vec!["fix", "survey"]);
    assert!(violations(&plan, &record).is_empty());
}

/// Rule 1. The answer on file was produced by a different prompt.
#[test]
fn editing_a_finished_node_is_refused_and_names_who_was_handed_its_answer() {
    let before = plan(SURVEY_AND_FIX);
    let record = build_record(&before, &outcome(all_done(&before)));
    let after = plan(&SURVEY_AND_FIX.replace(
        "List every file under src/ that still calls the old API.",
        "List every file under src/, including the tests.",
    ));

    assert_eq!(
        violations(&after, &record),
        vec![Violation::Edited {
            node: "survey".to_owned(),
            consumed_by: vec!["fix".to_owned()],
        }]
    );
}

/// The guard's actual advice, stated as a test: **edit the failure, not the
/// evidence.** A node that failed produced nothing to invalidate, and rewriting
/// it is the entire reason you came back to the plan.
#[test]
fn editing_a_failed_node_is_the_whole_point_of_coming_back() {
    let before = plan(SURVEY_AND_FIX);
    let record = build_record(
        &before,
        &outcome(states(&[
            ("survey", done("src/a.rs")),
            (
                "fix",
                NodeState::Failed {
                    conversation_id: "c-2".to_owned(),
                    reason: "the conversation ended `error`".to_owned(),
                },
            ),
        ])),
    );
    let after = plan(&SURVEY_AND_FIX.replace(
        "Migrate those files to the new API.",
        "Migrate those files to the new API. Do them one at a time.",
    ));

    assert!(
        violations(&after, &record).is_empty(),
        "a failed node is not evidence"
    );
    assert_eq!(sealed(&after, &record), vec!["survey"]);
}

/// Rule 2, and the reason the upstream walk exists at all: `b` and `c` are
/// untouched — their own definitions did not change — but the plan now runs
/// something in front of them that never ran, and a resume would skip them.
#[test]
fn a_node_inserted_upstream_of_finished_work_is_refused() {
    let before = plan(CHAIN);
    let record = build_record(&before, &outcome(all_done(&before)));
    let after = plan(
        r#"
        [[node]]
        id = "x"
        prompt = "x"
        [[node]]
        id = "a"
        prompt = "a"
        needs = ["x"]
        [[node]]
        id = "b"
        prompt = "b"
        needs = ["a"]
        [[node]]
        id = "c"
        prompt = "c"
        needs = ["b"]
        "#,
    );

    let violations = violations(&after, &record);
    assert_eq!(
        violations,
        vec![
            Violation::Edited {
                node: "a".to_owned(),
                consumed_by: Vec::new(),
            },
            Violation::ReachedBack {
                node: "b".to_owned(),
                upstream: "x".to_owned(),
            },
            Violation::ReachedBack {
                node: "c".to_owned(),
                upstream: "x".to_owned(),
            },
        ],
        "the reach-back travels through sealed work; only `a`'s own text changed"
    );
}

/// Only the nearest un-run ancestor is named, for the same reason
/// `newly_blocked` names the nearest blocker: a chain of new nodes is one
/// problem, not one per node.
#[test]
fn only_the_nearest_ancestor_that_never_ran_is_reported() {
    let before = plan(CHAIN);
    let record = build_record(&before, &outcome(all_done(&before)));
    let after = plan(
        r#"
        [[node]]
        id = "x"
        prompt = "x"
        [[node]]
        id = "y"
        prompt = "y"
        needs = ["x"]
        [[node]]
        id = "a"
        prompt = "a"
        needs = ["y"]
        [[node]]
        id = "b"
        prompt = "b"
        needs = ["a"]
        [[node]]
        id = "c"
        prompt = "c"
        needs = ["b"]
        "#,
    );

    let violations = violations(&after, &record);
    let reached_back: Vec<&str> = violations
        .iter()
        .filter_map(|violation| match violation {
            Violation::ReachedBack { upstream, .. } => Some(upstream.as_str()),
            Violation::Edited { .. } => None,
        })
        .collect();
    assert_eq!(
        reached_back,
        vec!["y", "y"],
        "`x` is behind `y` and adds nothing a reader could act on"
    );
}

/// There is no rule for a deleted node, and this is why: removing one rewrites
/// the `needs` of everything downstream, which is rule 1 on those nodes.
#[test]
fn deleting_a_finished_node_is_caught_through_its_dependent() {
    let before = plan(SURVEY_AND_FIX);
    let record = build_record(&before, &outcome(all_done(&before)));
    let after = plan(
        r#"
        [defaults]
        allow_tools = ["read-only"]

        [[node]]
        id = "fix"
        prompt = "Migrate those files to the new API."
        allow_tools = ["read-only", "APPLY_FILE_DIFFS"]
        "#,
    );

    assert_eq!(nodes_in(&violations(&after, &record)), vec!["fix"]);
}

/// `[defaults]` is resolved into the fingerprint rather than hashed beside it,
/// so loosening a default reaches exactly the nodes that inherit it.
#[test]
fn changing_a_default_reaches_the_nodes_that_inherit_it_and_no_others() {
    let before = plan(SURVEY_AND_FIX);
    let record = build_record(&before, &outcome(all_done(&before)));
    let after = plan(&SURVEY_AND_FIX.replace(
        r#"[defaults]
allow_tools = ["read-only"]"#,
        r#"[defaults]
allow_tools = ["read-only", "RUN_SHELL_COMMAND"]"#,
    ));

    assert_eq!(
        nodes_in(&violations(&after, &record)),
        vec!["survey"],
        "`fix` names its own allowlist, so the default never applied to it"
    );
}

/// The optional parts carry a `+`/`-` discriminant, so "no allowlist" and an
/// allowlist whose one entry happens to be `-` cannot hash the same.
#[test]
fn the_fingerprint_separates_an_absent_field_from_a_literal_dash() {
    let unrestricted = plan("[[node]]\nid = \"a\"\nprompt = \"a\"\n");
    let dash = plan("[[node]]\nid = \"a\"\nprompt = \"a\"\nallow_tools = [\"-\"]\n");
    assert_ne!(
        fingerprint(&unrestricted, &unrestricted.nodes[0]),
        fingerprint(&dash, &dash.nodes[0])
    );

    let ordering = plan(
        "[[node]]\nid = \"a\"\nprompt = \"a\"\n[[node]]\nid = \"b\"\nprompt = \"b\"\nneeds = [\"a\"]\n",
    );
    let passes_a_dash = plan(
        "[[node]]\nid = \"a\"\nprompt = \"a\"\n[[node]]\nid = \"b\"\nprompt = \"b\"\nneeds = [{ node = \"a\", pass = \"-\" }]\n",
    );
    assert_ne!(
        fingerprint(&ordering, &ordering.nodes[1]),
        fingerprint(&passes_a_dash, &passes_a_dash.nodes[1])
    );
}

/// Edge order counts, because `compose_prompt` appends handoffs in `needs`
/// order and the child reads them in that order.
#[test]
fn reordering_handoffs_changes_the_fingerprint() {
    let one = plan(
        r#"
        [[node]]
        id = "a"
        prompt = "a"
        [[node]]
        id = "b"
        prompt = "b"
        [[node]]
        id = "c"
        prompt = "c"
        needs = [{ node = "a", pass = "first" }, { node = "b", pass = "second" }]
        "#,
    );
    let other = plan(
        r#"
        [[node]]
        id = "a"
        prompt = "a"
        [[node]]
        id = "b"
        prompt = "b"
        [[node]]
        id = "c"
        prompt = "c"
        needs = [{ node = "b", pass = "second" }, { node = "a", pass = "first" }]
        "#,
    );

    assert_ne!(
        fingerprint(&one, &one.nodes[2]),
        fingerprint(&other, &other.nodes[2])
    );
}

/// An ordering edge carries nothing, so a node that only waited on the edited
/// one is not listed as having been handed anything.
#[test]
fn an_ordering_edge_is_not_a_consumer() {
    let before = plan(CHAIN);
    let record = build_record(&before, &outcome(all_done(&before)));
    let after = plan(&CHAIN.replace(r#"prompt = "a""#, r#"prompt = "a, differently""#));

    assert_eq!(
        violations(&after, &record),
        vec![Violation::Edited {
            node: "a".to_owned(),
            consumed_by: Vec::new(),
        }]
    );
}

/// What a resume actually does, without a socket: the finished node is not
/// ready to run again, the next one is, and the recorded answer reaches it.
#[test]
fn a_resume_skips_what_finished_and_hands_its_answer_on() {
    let plan = plan(SURVEY_AND_FIX);
    let record = build_record(
        &plan,
        &outcome(states(&[
            ("survey", done("src/a.rs\nsrc/b.rs")),
            (
                "fix",
                NodeState::Failed {
                    conversation_id: "c-2".to_owned(),
                    reason: "the conversation ended `error`".to_owned(),
                },
            ),
        ])),
    );

    let reuse = reusable(&plan, &record);
    assert_eq!(
        reuse.states.keys().collect::<Vec<_>>(),
        vec!["survey"],
        "only a finished node has an answer worth reusing"
    );
    assert_eq!(
        ready(&plan, &reuse.states, 0, 4)
            .iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>(),
        vec!["fix"],
        "the survey is not spawned a second time"
    );
    assert!(
        compose_prompt(&plan.nodes[1], &reuse.states, None).contains("src/a.rs\nsrc/b.rs"),
        "and the answer it already gave still reaches the node that needed it"
    );
}

/// A node the record has never heard of is simply pending — a plan may grow.
#[test]
fn a_node_the_record_does_not_know_is_not_reused() {
    let before = plan(SURVEY_AND_FIX);
    let record = build_record(&before, &outcome(all_done(&before)));
    let after = plan(&format!(
        "{SURVEY_AND_FIX}\n[[node]]\nid = \"report\"\nprompt = \"report\"\nneeds = [\"fix\"]\n"
    ));

    assert!(
        violations(&after, &record).is_empty(),
        "adding work after finished work invalidates nothing"
    );
    assert_eq!(reusable(&after, &record).states.len(), 2);
}

/// The schema is the format's only documentation, and it now has to mention the
/// half of the format that is not fields.
#[test]
fn the_schema_documents_the_record_and_the_guard() {
    for phrase in [
        "--resume",
        "--no-record",
        "plan.toml.run.json",
        ".gitignore",
    ] {
        assert!(
            SCHEMA.contains(phrase),
            "the schema should mention `{phrase}`"
        );
    }
}

// ---------------------------------------------------------------------------
// Acceptance assertions (`.fork/TASKS.md`, T13.2).
//
// The ones that run a command are `#[cfg(unix)]`: `sh -c` is the shell on every
// machine this fork is developed on, and pinning `cmd /C` spellings on a
// platform nobody has run this on would be asserting a guess. The Windows path
// is named as unverified in the as-built rather than covered by a test that
// only proves the test was written.
// ---------------------------------------------------------------------------

const ASSERTED: &str = r#"
[[node]]
id = "fix"
prompt = "Migrate the files."
assert = [
  { id = "compiles", run = "true" },
  { id = "clean", run = "true" },
]
"#;

/// Both spellings, one concept — the same shape `needs` has.
#[test]
fn an_assertion_can_name_itself_or_be_named() {
    let plan = plan(
        r#"
        [[node]]
        id = "a"
        prompt = "a"
        assert = ["cargo check --quiet", { id = "clean", run = "git diff --quiet" }]
        "#,
    );

    assert!(validate(&plan).is_ok());
    assert_eq!(plan.nodes[0].assertions.len(), 2);
    assert_eq!(plan.nodes[0].assertions[0].id(), "cargo check --quiet");
    assert_eq!(plan.nodes[0].assertions[0].run(), "cargo check --quiet");
    assert_eq!(plan.nodes[0].assertions[1].id(), "clean");
    assert_eq!(plan.nodes[0].assertions[1].run(), "git diff --quiet");
}

/// All that is left of Zenith's coverage invariant.
///
/// "Exactly one active owner per assertion" has two failure modes there —
/// un-owned and doubly-owned — and neither is expressible here, because an
/// assertion is written inside the node that owns it. What remains is that a
/// verdict has to be able to name which assertion it is about.
#[test]
fn a_node_cannot_assert_the_same_thing_twice() {
    let twice = plan(
        r#"
        [[node]]
        id = "a"
        prompt = "a"
        assert = [{ id = "same", run = "true" }, { id = "same", run = "false" }]
        "#,
    );

    let error = validate(&twice).expect_err("two verdicts could not be told apart");
    assert!(
        error.message.contains("asserts `same` twice"),
        "{}",
        error.message
    );

    let empty = plan(
        r#"
        [[node]]
        id = "a"
        prompt = "a"
        assert = [{ id = "nothing", run = "   " }]
        "#,
    );
    assert!(
        validate(&empty)
            .expect_err("an assertion with no command asserts nothing")
            .message
            .contains("nothing to run")
    );
}

/// Two nodes may assert the same id — a verdict is filed under its node.
#[test]
fn the_same_assertion_id_on_two_nodes_is_fine() {
    let plan = plan(
        r#"
        [[node]]
        id = "a"
        prompt = "a"
        assert = [{ id = "compiles", run = "true" }]
        [[node]]
        id = "b"
        prompt = "b"
        assert = [{ id = "compiles", run = "true" }]
        "#,
    );

    assert!(validate(&plan).is_ok());
}

/// Loosening an assertion is an edit to what "done" meant, and T13.1's guard
/// would otherwise be blind to the one change that matters most.
#[test]
fn changing_an_assertion_changes_the_fingerprint() {
    let strict = plan(ASSERTED);
    let loosened = plan(&ASSERTED.replace(r#"run = "true" "#, r#"run = "true || true" "#));
    let dropped = plan(
        r#"
        [[node]]
        id = "fix"
        prompt = "Migrate the files."
        "#,
    );

    let strict_print = fingerprint(&strict, &strict.nodes[0]);
    assert_ne!(strict_print, fingerprint(&loosened, &loosened.nodes[0]));
    assert_ne!(
        strict_print,
        fingerprint(&dropped, &dropped.nodes[0]),
        "deleting the gate is the loosest edit of all"
    );
}

/// A node with no assertions gets no verdicts and no gate — the pre-T13.2
/// behaviour, unchanged.
#[test]
fn a_node_that_asserts_nothing_is_judged_by_nothing() {
    let plan = plan(SURVEY_AND_FIX);

    assert!(evaluate(&plan.nodes[0], "anything").is_empty());
}

#[cfg(unix)]
#[test]
fn every_assertion_is_run_and_reported_separately() {
    let plan = plan(
        r#"
        [[node]]
        id = "a"
        prompt = "a"
        assert = [
          { id = "passes", run = "true" },
          { id = "fails", run = "echo the tree does not build >&2; exit 3" },
          { id = "also-passes", run = "true" },
        ]
        "#,
    );

    let verdicts = evaluate(&plan.nodes[0], "");
    assert_eq!(
        verdicts.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(),
        vec!["passes", "fails", "also-passes"],
        "all of them, in the order they were written — stopping at the first \
         failure answers a different question"
    );
    assert_eq!(
        verdicts.iter().map(|v| v.passed).collect::<Vec<_>>(),
        vec![true, false, true]
    );
    assert_eq!(verdicts[1].code, Some(3));
    assert_eq!(
        verdicts[1].detail, "the tree does not build",
        "a failing check that explained itself is the whole value"
    );
    assert!(
        verdicts[0].detail.is_empty(),
        "a passing check has nothing to say"
    );
}

/// The node's answer is on stdin, so an assertion can be about the answer as
/// well as about the world.
#[cfg(unix)]
#[test]
fn the_nodes_answer_reaches_the_assertion_on_stdin() {
    let plan = plan(
        r#"
        [[node]]
        id = "survey"
        prompt = "list them"
        assert = [{ id = "one-per-line", run = "grep -q '^src/'" }]
        "#,
    );

    assert!(evaluate(&plan.nodes[0], "src/a.rs\nsrc/b.rs")[0].passed);
    assert!(!evaluate(&plan.nodes[0], "I could not find any files.")[0].passed);
}

/// And its id is in the environment, for an assertion shared across nodes.
#[cfg(unix)]
#[test]
fn the_node_id_reaches_the_assertion_in_the_environment() {
    let plan = plan(
        r#"
        [[node]]
        id = "survey"
        prompt = "list them"
        assert = [{ id = "knows-me", run = "test \"$WARP_GRAPH_NODE\" = survey" }]
        "#,
    );

    assert!(evaluate(&plan.nodes[0], "")[0].passed);
}

/// A command that cannot run at all is a failed assertion, not a crashed run.
#[cfg(unix)]
#[test]
fn an_assertion_that_cannot_run_fails_rather_than_stopping_the_plan() {
    let plan = plan(
        r#"
        [[node]]
        id = "a"
        prompt = "a"
        assert = [{ id = "missing", run = "definitely-not-a-real-binary-9f3a" }]
        "#,
    );

    let verdict = &evaluate(&plan.nodes[0], "")[0];
    assert!(!verdict.passed);
    assert_eq!(
        verdict.code,
        Some(127),
        "the shell starts fine; it is the command inside that is missing"
    );
}

/// A child that never reads stdin must not hang the run.
///
/// Pinned because it is the failure the three-thread shape in `verdict_for`
/// exists to prevent, and because it only shows up once the answer is bigger
/// than a pipe buffer — which is exactly the size a real agent answer is.
#[cfg(unix)]
#[test]
fn an_assertion_that_ignores_a_large_answer_does_not_deadlock() {
    let plan = plan(
        r#"
        [[node]]
        id = "a"
        prompt = "a"
        assert = [{ id = "ignores-stdin", run = "true" }]
        "#,
    );

    let huge = "x".repeat(1_000_000);
    assert!(evaluate(&plan.nodes[0], &huge)[0].passed);
}

/// A child that writes more than a pipe buffer must not hang either.
#[cfg(unix)]
#[test]
fn an_assertion_that_says_a_great_deal_does_not_deadlock() {
    let plan = plan(
        r#"
        [[node]]
        id = "a"
        prompt = "a"
        assert = [{ id = "verbose", run = "yes nonsense | head -c 1000000; exit 1" }]
        "#,
    );

    let verdict = &evaluate(&plan.nodes[0], "")[0];
    assert!(!verdict.passed);
    assert_eq!(verdict.detail, "nonsense", "trimmed to its first line");
}

/// Rejected is not failed, and the difference is what a reader does next.
#[test]
fn a_rejected_node_blocks_its_dependents_exactly_as_a_failure_does() {
    let plan = plan(SURVEY_AND_FIX);
    let rejected = states(&[(
        "survey",
        NodeState::Rejected {
            conversation_id: "c-1".to_owned(),
            output: "I did not find any.".to_owned(),
        },
    )]);

    assert_eq!(
        newly_blocked(&plan, &rejected),
        vec![("fix".to_owned(), "survey".to_owned())]
    );
    assert!(
        ready(&plan, &rejected, 0, 4).is_empty(),
        "a rejected node's answer must not reach the node that needed it"
    );
}

/// It is settled, so it is recorded — and it is not `Done`, so it is not sealed
/// and `--resume` runs it again. Both halves matter.
#[test]
fn a_rejected_node_is_recorded_but_never_reused() {
    let plan = plan(ASSERTED);
    let record = build_record(
        &plan,
        &Outcome {
            states: states(&[(
                "fix",
                NodeState::Rejected {
                    conversation_id: "c-1".to_owned(),
                    output: "I changed one file.".to_owned(),
                },
            )]),
            verdicts: [(
                "fix".to_owned(),
                vec![
                    Verdict {
                        id: "compiles".to_owned(),
                        passed: false,
                        code: Some(101),
                        detail: "error[E0425]: cannot find value `old_api`".to_owned(),
                    },
                    Verdict {
                        id: "clean".to_owned(),
                        passed: true,
                        code: Some(0),
                        detail: String::new(),
                    },
                ],
            )]
            .into_iter()
            .collect(),
        },
    );

    assert_eq!(record.nodes["fix"].verdicts.len(), 2);
    assert_eq!(
        record.nodes["fix"].verdicts[0].detail, "error[E0425]: cannot find value `old_api`",
        "which assertion failed, and why, is the point of recording per-assertion"
    );
    assert!(
        sealed(&plan, &record).is_empty(),
        "work that did not hold up is not evidence"
    );
    assert!(
        reusable(&plan, &record).states.is_empty(),
        "so a resume runs it again"
    );
    assert!(
        violations(&plan, &record).is_empty(),
        "and editing it is not a violation — that is the fix"
    );
}

/// Verdicts travel with the node they belong to, and are not re-run.
#[test]
fn a_reused_node_carries_the_verdicts_it_earned() {
    let plan = plan(ASSERTED);
    let record = build_record(
        &plan,
        &Outcome {
            states: states(&[("fix", done("changed three files"))]),
            verdicts: [(
                "fix".to_owned(),
                vec![Verdict {
                    id: "compiles".to_owned(),
                    passed: true,
                    code: Some(0),
                    detail: String::new(),
                }],
            )]
            .into_iter()
            .collect(),
        },
    );

    let reuse = reusable(&plan, &record);
    assert_eq!(reuse.states.len(), 1);
    assert_eq!(
        reuse.verdicts["fix"][0].id, "compiles",
        "a resume that forgot the verdicts would report a gate that never ran"
    );
}

/// The record round-trips a verdict, including the absent-on-pass detail.
#[test]
fn verdicts_survive_the_record_file() {
    let plan = plan(ASSERTED);
    let record = build_record(
        &plan,
        &Outcome {
            states: states(&[("fix", done("ok"))]),
            verdicts: [(
                "fix".to_owned(),
                vec![Verdict {
                    id: "compiles".to_owned(),
                    passed: true,
                    code: Some(0),
                    detail: String::new(),
                }],
            )]
            .into_iter()
            .collect(),
        },
    );

    let text = serde_json::to_string(&record).expect("a record should serialize");
    assert!(
        !text.contains("detail"),
        "an empty detail is left out rather than written as \"\": {text}"
    );
    let parsed: RunRecord = serde_json::from_str(&text).expect("and parse back");
    assert_eq!(parsed.nodes["fix"].verdicts[0].code, Some(0));
}

/// A node with no assertions writes no `verdicts` key at all.
#[test]
fn a_node_without_assertions_adds_nothing_to_the_record() {
    let plan = plan(SURVEY_AND_FIX);
    let record = build_record(&plan, &outcome(all_done(&plan)));

    let text = serde_json::to_string(&record).expect("a record should serialize");
    assert!(!text.contains("verdicts"), "{text}");
}

/// The schema is the format's only documentation.
#[test]
fn the_schema_documents_assertions() {
    for phrase in ["assert = [", "$WARP_GRAPH_NODE", "rejected", "stdin"] {
        assert!(
            SCHEMA.contains(phrase),
            "the schema should mention `{phrase}`"
        );
    }
}

// ---------------------------------------------------------------------------
// The review fence (`.fork/TASKS.md`, T13.3 — `ZB-REVIEW`).
//
// There is no reviewer to test, because a review is an ordinary `agent.spawn`
// and its independence is a property of that primitive rather than of anything
// here. What is testable is the fence: the three silent ways to turn a reviewer
// into a rubber stamp, each refused before a token is spent.
// ---------------------------------------------------------------------------

const REVIEWED: &str = r#"
[[node]]
id = "fix"
prompt = "Migrate the files."

[[node]]
id = "review"
review = true
prompt = "Read the workspace and say whether the goal is met."
needs = ["fix"]
assert = [{ id = "no-gaps", run = "grep -qx 'NO GAPS FOUND'" }]
"#;

#[test]
fn a_review_node_is_an_ordinary_node_that_waits_for_the_work() {
    let plan = plan(REVIEWED);

    validate(&plan).expect("the recipe should be a runnable plan");
    assert!(plan.nodes[1].review);
    assert_eq!(
        waves(&plan),
        vec![vec!["fix".to_owned()], vec!["review".to_owned()]]
    );
    assert_eq!(
        compose_prompt(
            &plan.nodes[1],
            &states(&[("fix", done("I migrated everything."))]),
            None,
        ),
        "Read the workspace and say whether the goal is met.",
        "an ordering edge hands nothing along, which is the whole of the \
         independence — the reviewer never sees what `fix` said"
    );
}

/// The important one. A handoff would append the worker's own account of what
/// it did to the reviewer's prompt, which is precisely the claim-about-a-claim
/// the gate exists to avoid.
#[test]
fn a_review_may_not_be_handed_the_answer_it_is_reviewing() {
    let plan = plan(&REVIEWED.replace(
        r#"needs = ["fix"]"#,
        r#"needs = [{ node = "fix", pass = "what I did" }]"#,
    ));

    let error = validate(&plan).expect_err("a reviewer told what was done grades a claim");
    assert!(
        error
            .message
            .contains("grading a claim rather than the work"),
        "{}",
        error.message
    );
}

/// A `needs = [{ node = "fix" }]` with no `pass` is ordering, and allowed —
/// the refusal is about the payload, not the spelling.
#[test]
fn the_long_spelling_of_an_ordering_edge_is_still_ordering() {
    let plan = plan(&REVIEWED.replace(r#"needs = ["fix"]"#, r#"needs = [{ node = "fix" }]"#));

    assert!(validate(&plan).is_ok());
}

/// A reviewer that can write can make its own verdict true.
#[test]
fn a_review_may_not_name_its_own_tools() {
    let plan = plan(&REVIEWED.replace(
        r#"review = true"#,
        r#"review = true
allow_tools = ["read-only", "APPLY_FILE_DIFFS"]"#,
    ));

    let error = validate(&plan).expect_err("a review is always read-only");
    assert!(
        error.message.contains("can make its own verdict true"),
        "{}",
        error.message
    );
}

/// Refused rather than silently corrected, and refused even when what it names
/// is harmless — there is one right answer, and offering the choice is what
/// invites the wrong one.
#[test]
fn a_review_is_refused_even_for_naming_read_only_itself() {
    let plan = plan(&REVIEWED.replace(
        r#"review = true"#,
        r#"review = true
allow_tools = ["read-only"]"#,
    ));

    assert!(validate(&plan).is_err());
}

/// And it resolves to read-only regardless of what `[defaults]` says, which is
/// the case a fence over `allow_tools` alone would have missed.
#[test]
fn a_review_ignores_a_wide_default_allowlist() {
    let plan = plan(&format!(
        "[defaults]\nallow_tools = [\"RUN_SHELL_COMMAND\", \"APPLY_FILE_DIFFS\"]\n{REVIEWED}"
    ));

    validate(&plan).expect("a wide default is fine for the working nodes");
    let review = spawn_params(&plan, &plan.nodes[1], &states(&[]), None, None);
    assert_eq!(
        review.allow_tools,
        Some(vec!["read-only".to_owned()]),
        "the default reaches every node except the one that must not have it"
    );
    let fix = spawn_params(&plan, &plan.nodes[0], &states(&[]), None, None);
    assert_eq!(
        fix.allow_tools,
        Some(vec![
            "RUN_SHELL_COMMAND".to_owned(),
            "APPLY_FILE_DIFFS".to_owned()
        ])
    );
}

/// Its input is the working tree, which every node shares — so a review that
/// could start while anything else is still running reviews a workspace
/// mid-edit, and does it differently every time.
#[test]
fn a_review_must_wait_for_every_working_node() {
    let plan = plan(
        r#"
        [[node]]
        id = "fix"
        prompt = "Migrate the files."
        [[node]]
        id = "docs"
        prompt = "Write the docs."
        [[node]]
        id = "review"
        review = true
        prompt = "Read the workspace."
        needs = ["fix"]
        "#,
    );

    let error = validate(&plan).expect_err("`docs` could still be writing");
    assert!(
        error.message.contains("does not wait for `docs`"),
        "{}",
        error.message
    );
}

/// Transitively, not directly — a review at the end of a chain has waited for
/// all of it.
#[test]
fn waiting_for_the_last_node_of_a_chain_is_waiting_for_the_chain() {
    let plan = plan(&format!(
        "{CHAIN}\n[[node]]\nid = \"review\"\nreview = true\nprompt = \"Read it.\"\nneeds = [\"c\"]\n"
    ));

    assert!(validate(&plan).is_ok());
}

/// Two reviewers with different lenses are fine, and neither has to wait for
/// the other.
#[test]
fn two_reviews_may_sit_side_by_side() {
    let plan = plan(
        r#"
        [[node]]
        id = "fix"
        prompt = "Migrate the files."
        [[node]]
        id = "product-review"
        review = true
        prompt = "Does it do what was asked?"
        needs = ["fix"]
        [[node]]
        id = "test-review"
        review = true
        prompt = "Is it tested?"
        needs = ["fix"]
        "#,
    );

    validate(&plan).expect("a review need not wait for another review");
    assert_eq!(waves(&plan).len(), 2);
}

/// Turning the reviewer into an ordinary node is one word, and it is an edit to
/// what its answer meant — so T13.1's guard has to see it.
#[test]
fn unmarking_a_review_changes_the_fingerprint() {
    let reviewed = plan(REVIEWED);
    let plain = plan(&REVIEWED.replace("review = true\n", ""));

    assert_ne!(
        fingerprint(&reviewed, &reviewed.nodes[1]),
        fingerprint(&plain, &plain.nodes[1])
    );
}

/// The schema carries the recipe, because the recipe *is* the deliverable.
#[test]
fn the_schema_documents_the_review_recipe() {
    for phrase in [
        "review = true",
        "NO GAPS FOUND",
        "can only usefully FAIL",
        "inherits no transcript",
    ] {
        assert!(
            SCHEMA.contains(phrase),
            "the schema should mention `{phrase}`"
        );
    }
}

/// A reviewer is told which tree it is reviewing, because it did not start in
/// it.
///
/// Found by running one: `agent.spawn` takes no working directory, so the child
/// starts in the *pane's* cwd. The first live review read
/// `/home/effatha/git/warp` while the plan was being run from `/tmp/t133/work`,
/// reported "there is no ./src directory in the working tree", and failed its
/// gate for that instead of for the gap that had been planted for it.
#[test]
fn a_review_is_told_where_the_workspace_is() {
    let plan = plan(REVIEWED);
    let workspace = Path::new("/tmp/somewhere/else");

    let review = compose_prompt(&plan.nodes[1], &states(&[]), Some(workspace));
    assert!(
        review.contains("`/tmp/somewhere/else`"),
        "the reviewer's only input is the tree, so it has to be told which one: {review}"
    );
    assert!(
        review.contains("a relative path will silently resolve somewhere else"),
        "and told why, or it will use a relative one anyway: {review}"
    );

    let worker = compose_prompt(&plan.nodes[0], &states(&[]), Some(workspace));
    assert_eq!(
        worker, "Migrate the files.",
        "no other node is told, because every other node's input is its prompt"
    );
}

/// The directory is environment, not plan. Two runs of one plan from two places
/// are the same plan, exactly as `assert = [\"cargo check\"]` is.
#[test]
fn the_workspace_is_not_part_of_the_fingerprint() {
    let plan = plan(REVIEWED);

    assert_eq!(
        fingerprint(&plan, &plan.nodes[1]),
        fingerprint(&plan, &plan.nodes[1]),
        "nothing environmental may reach it"
    );
    assert!(
        !fingerprint(&plan, &plan.nodes[1]).is_empty(),
        "and it is still a fingerprint"
    );
}
