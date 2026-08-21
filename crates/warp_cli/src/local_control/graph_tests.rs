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

    let survey = spawn_params(&plan, &plan.nodes[0], &states, None);
    assert_eq!(
        survey.allow_tools.as_deref(),
        Some(&["read-only".to_owned()][..])
    );

    let fix = spawn_params(&plan, &plan.nodes[1], &states, Some("parent".to_owned()));
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

    let prompt = compose_prompt(&plan.nodes[1], &states);
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

    assert_eq!(compose_prompt(&plan.nodes[1], &states), "second");
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
