//! A run-scale task graph, and the loop that runs it (`.fork/TASKS.md`, T7.1).
//!
//! # Why this is a file and a loop rather than a feature of Warp
//!
//! Warp can already do all of this at runtime — `RUN_AGENTS`, `SUBAGENT`,
//! `SEND_MESSAGE_TO_AGENT` and `WAIT_FOR_EVENTS` are a message-passing
//! substrate, and "B waits for A, then A hands B its result" is expressible in
//! it today. What is missing is not the mechanism. It is that the sequencing is
//! a decision the model makes in the moment rather than a declaration made
//! before the run.
//!
//! So the plan is a document, and this is a `while` loop over `agent.spawn`
//! and `agent.read`. Nothing here is new app surface: the action count is the
//! same after T7.1 as before it. That is the point — T6.6 built the verbs, and
//! a graph is a composition of them.
//!
//! The reason it is a *file* is durability. A plan held in the lead agent's
//! context degrades exactly as the work gets long enough to need it, and
//! compaction is the moment it is most at risk. A file is also diffable,
//! reviewable, and lands in a commit next to the work it describes.
//!
//! # The shape
//!
//! ```toml
//! [[node]]
//! id = "survey"
//! prompt = "List every file under src/ that still calls the old API."
//! allow_tools = ["read-only"]
//!
//! [[node]]
//! id = "fix"
//! prompt = "Migrate those files to the new API."
//! needs = [{ node = "survey", pass = "the list of files" }]
//! ```
//!
//! One edge type, because a dependency *is* an edge that carries a payload:
//! `hands-to` is `depends-on` plus "and here is what to pass". Two edge types
//! would have to be kept consistent, and a graph where B hands to C but C does
//! not depend on B is a bug you can draw.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use local_control::protocol::{
    ActionKind, AgentReadParams, AgentReadResult, AgentSpawnParams, AgentSpawnResult, ControlError,
    ErrorCode,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::agent::OutputFormat;
use crate::local_control::output::{write_json, write_json_line};
use crate::local_control::{GraphCheckArgs, GraphCommand, GraphRunArgs, TargetArgs};

/// A plan, as it is written on disk.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Plan {
    /// Applied to any node that does not say otherwise.
    #[serde(default)]
    pub defaults: Defaults,
    /// Written `[[node]]`, so the plural reads right in the file.
    #[serde(default, rename = "node")]
    pub nodes: Vec<Node>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Defaults {
    /// The tool allowlist every node inherits. `None` is "no policy", which is
    /// what an unrestricted agent has — see `agent.spawn --allow-tools`.
    #[serde(default)]
    pub allow_tools: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Node {
    /// How edges name this node. Unique within a plan.
    pub id: String,
    pub prompt: String,
    /// The child agent's name in Warp. Defaults to the id.
    #[serde(default)]
    pub name: Option<String>,
    /// Overrides [`Defaults::allow_tools`] when present.
    #[serde(default)]
    pub allow_tools: Option<Vec<String>>,
    #[serde(default)]
    pub needs: Vec<Need>,
    /// What must hold once this node has finished (`.fork/TASKS.md`, T13.2).
    #[serde(default, rename = "assert")]
    pub assertions: Vec<Assertion>,
}

/// One thing that must be true once a node's turn is over.
///
/// A command, and not a sentence, because the whole point of an acceptance
/// contract is that it is *falsifiable*: **the statement and the evidence are
/// the same string.** A node that says "the tests pass" is making a claim, and
/// asking a second model whether the first model's claim is true is a claim
/// about a claim. `cargo test --quiet` cannot be talked around.
///
/// Two spellings and one concept, exactly as [`Need`] has: the bare form is the
/// one that gets written, and the named form exists for when the command is too
/// long to read as a label.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(super) enum Assertion {
    /// `assert = ["cargo check --quiet"]` — the command is its own name.
    Command(String),
    /// `assert = [{ id = "compiles", run = "cargo check --quiet" }]`.
    Named { id: String, run: String },
}

impl Assertion {
    fn id(&self) -> &str {
        match self {
            Self::Command(run) => run,
            Self::Named { id, .. } => id,
        }
    }

    fn run(&self) -> &str {
        match self {
            Self::Command(run) | Self::Named { run, .. } => run,
        }
    }
}

/// What one assertion turned out to be.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Verdict {
    pub id: String,
    pub passed: bool,
    /// The exit status, or `None` when the command never reached one — it could
    /// not be started, or it outlived [`ASSERT_TIMEOUT`].
    pub code: Option<i32>,
    /// The first line it had to say for itself. Empty on a pass, because a
    /// passing check has nothing to report and a record full of noise is a
    /// record nobody reads.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub detail: String,
}

/// One edge, pointing backwards at what has to happen first.
///
/// Edges are written on the node that waits, not on the node that runs first,
/// because that is the direction a reader asks the question in: standing at
/// `fix`, what does it need? The graph is the same either way.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(super) enum Need {
    /// `needs = ["survey"]` — ordering, with nothing handed along.
    Ordering(String),
    /// `needs = [{ node = "survey", pass = "the list of files" }]`.
    Handoff {
        node: String,
        /// What the upstream node's answer *is*, in one phrase, appended to
        /// this node's prompt above the answer itself. Not a filter: the whole
        /// output is passed either way. It is a label, because a wall of text
        /// under no heading is the kind of context an agent quietly ignores.
        #[serde(default)]
        pass: Option<String>,
    },
}

impl Need {
    fn node(&self) -> &str {
        match self {
            Self::Ordering(node) => node,
            Self::Handoff { node, .. } => node,
        }
    }

    fn pass(&self) -> Option<&str> {
        match self {
            Self::Ordering(_) => None,
            Self::Handoff { pass, .. } => pass.as_deref(),
        }
    }
}

impl Node {
    fn name(&self) -> String {
        self.name.clone().unwrap_or_else(|| self.id.clone())
    }

    fn allow_tools(&self, defaults: &Defaults) -> Option<Vec<String>> {
        self.allow_tools
            .clone()
            .or_else(|| defaults.allow_tools.clone())
    }
}

/// Where a node has got to.
///
/// `Skipped` is deliberately distinct from `Failed`. A run that reports six
/// failures when one node failed and five were waiting on it has hidden the
/// only fact worth acting on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(super) enum NodeState {
    Pending,
    Running {
        conversation_id: String,
    },
    Done {
        conversation_id: String,
        output: String,
    },
    Failed {
        conversation_id: String,
        reason: String,
    },
    /// The turn finished, and the work it produced did not hold up.
    ///
    /// A fifth state rather than a second kind of `Failed`, for the same reason
    /// `Skipped` is not `Failed`: a reader acts differently on each. *Failed* is
    /// "the agent could not do it" and the answer is usually to run it again.
    /// *Rejected* is "the agent said it was done and an assertion says
    /// otherwise" — running it again unchanged will produce the same thing, and
    /// what needs editing is the prompt or the assertion. The output is kept
    /// because the claim is the evidence you debug from.
    Rejected {
        conversation_id: String,
        output: String,
    },
    /// Never started, because something it needs did not finish.
    Skipped {
        blocked_by: String,
    },
}

impl NodeState {
    fn is_settled(&self) -> bool {
        !matches!(self, Self::Pending | Self::Running { .. })
    }
}

/// Validates a plan before anything is spawned.
///
/// Every check here is one that would otherwise be discovered halfway through a
/// run, with children already running and a partial result to reason about.
/// A cycle in particular is invisible at runtime — it presents as a scheduler
/// that simply stops finding work, which reads like a hang.
pub(super) fn validate(plan: &Plan) -> Result<(), ControlError> {
    if plan.nodes.is_empty() {
        return Err(invalid("the plan has no `[[node]]` entries"));
    }

    let mut seen = HashSet::new();
    for node in &plan.nodes {
        if node.id.trim().is_empty() {
            return Err(invalid("every node needs a non-empty `id`"));
        }
        if node.prompt.trim().is_empty() {
            return Err(invalid(format!("node `{}` has an empty `prompt`", node.id)));
        }
        if !seen.insert(node.id.as_str()) {
            return Err(invalid(format!(
                "two nodes share the id `{}`; edges could not tell them apart",
                node.id
            )));
        }

        let mut asserted = HashSet::new();
        for assertion in &node.assertions {
            if assertion.run().trim().is_empty() {
                return Err(invalid(format!(
                    "node `{}` has an assertion with nothing to run",
                    node.id
                )));
            }
            if !asserted.insert(assertion.id()) {
                return Err(invalid(format!(
                    "node `{}` asserts `{}` twice; a verdict could not tell them apart",
                    node.id,
                    assertion.id()
                )));
            }
        }
    }

    for node in &plan.nodes {
        for need in &node.needs {
            if need.node() == node.id {
                return Err(invalid(format!("node `{}` needs itself", node.id)));
            }
            if !seen.contains(need.node()) {
                return Err(invalid(format!(
                    "node `{}` needs `{}`, which is not in this plan",
                    node.id,
                    need.node()
                )));
            }
        }
    }

    if let Some(cycle) = find_cycle(plan) {
        return Err(invalid(format!(
            "these nodes wait on each other and none of them could ever start: {}",
            cycle.join(" -> ")
        )));
    }

    Ok(())
}

/// The nodes in a cycle, if there is one.
///
/// Reports the members rather than just "there is a cycle", because a plan big
/// enough to have one by accident is big enough that finding it by eye is the
/// actual work.
fn find_cycle(plan: &Plan) -> Option<Vec<String>> {
    // Kahn's algorithm, and what it *cannot* remove is the answer: a node whose
    // dependencies never all clear is either in a cycle or downstream of one.
    let mut remaining: HashMap<&str, HashSet<&str>> = plan
        .nodes
        .iter()
        .map(|node| {
            (
                node.id.as_str(),
                node.needs.iter().map(Need::node).collect::<HashSet<_>>(),
            )
        })
        .collect();

    loop {
        let ready: Vec<&str> = remaining
            .iter()
            .filter(|(_, needs)| needs.is_empty())
            .map(|(id, _)| *id)
            .collect();
        if ready.is_empty() {
            break;
        }
        for id in ready {
            remaining.remove(id);
            for needs in remaining.values_mut() {
                needs.remove(id);
            }
        }
    }

    if remaining.is_empty() {
        return None;
    }
    // Stable order, so the message is the same on every run and diffable.
    let mut cycle: Vec<String> = remaining.keys().map(|id| (*id).to_owned()).collect();
    cycle.sort();
    Some(cycle)
}

// ---------------------------------------------------------------------------
// Acceptance assertions (`.fork/TASKS.md`, T13.2 — `ZB-CONTRACT`).
//
// A node's turn ending `success` means the agent stopped without erroring. It
// does not mean the work happened. These run after that, in the directory
// `graph run` was invoked from, with the node's answer on **stdin** and its id
// in `WARP_GRAPH_NODE` — so an assertion can be about the world (`cargo check`)
// or about the answer (`grep -q '^src/'`), and usually should be about the
// world.
//
// **The coverage invariant collapses here, and for the same reason the sealed
// subgraph did in T13.1.** Zenith's contract lives beside the plan and its
// assertions are *claimed* by tasks, so "exactly one active owner per
// assertion" is a real invariant with two real failure modes — un-owned, and
// doubly-owned. Here an assertion is written inside the node that owns it, so
// it has exactly one owner by construction and neither failure mode is
// expressible. What is left of the invariant is that two assertions on one node
// must not share an id, which `validate` refuses. This is the second time the
// fork's habit of writing a relationship on the thing itself rather than in a
// side table has deleted an invariant rather than implementing it.
// ---------------------------------------------------------------------------

/// How long an assertion gets before it is killed and counted as failed.
///
/// Fixed rather than configurable because an assertion is a *check*, not the
/// work — a plan that needs longer than this to decide whether its own node
/// succeeded has put the work in the wrong place. The reason there is a limit
/// at all is that the whole point of a run gate is unattended runs, and a gate
/// that can hang forever is not one.
const ASSERT_TIMEOUT: Duration = Duration::from_secs(120);

/// Runs every assertion on a node and reports each one separately.
///
/// All of them, not up to the first failure: "which of the three things I asked
/// for actually happened" is the question this exists to answer, and stopping
/// early answers a different one. They are commands, so the cost of running the
/// rest is nothing.
pub(super) fn evaluate(node: &Node, output: &str) -> Vec<Verdict> {
    node.assertions
        .iter()
        .map(|assertion| verdict_for(assertion, &node.id, output))
        .collect()
}

/// One assertion, run.
fn verdict_for(assertion: &Assertion, node: &str, output: &str) -> Verdict {
    let id = assertion.id().to_owned();
    let mut command = shell(assertion.run());
    command
        .env("WARP_GRAPH_NODE", node)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return Verdict {
                id,
                passed: false,
                code: None,
                detail: format!("could not be started: {error}"),
            };
        }
    };

    // Every pipe gets its own thread, and this is not belt-and-braces. A child
    // that never reads stdin blocks *us* once the pipe fills, and a child that
    // writes more than a pipeful blocks *itself* while we are polling for its
    // exit — either one is a hang, and a `cargo check` on a broken tree emits
    // far more than a pipeful.
    let mut stdin = child.stdin.take();
    let payload = output.to_owned();
    let writer = std::thread::spawn(move || {
        if let Some(stdin) = stdin.as_mut() {
            // Ignored: a command that does not read stdin is not wrong, it just
            // hands us an `EPIPE` when it exits.
            let _ = std::io::Write::write_all(stdin, payload.as_bytes());
        }
    });
    let mut stdout = child.stdout.take();
    let out = std::thread::spawn(move || drain(stdout.as_mut()));
    let mut stderr = child.stderr.take();
    let err = std::thread::spawn(move || drain(stderr.as_mut()));

    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => {
                if started.elapsed() > ASSERT_TIMEOUT {
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err(format!("still running after {}s", ASSERT_TIMEOUT.as_secs()));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(error) => break Err(format!("could not be waited for: {error}")),
        }
    };

    // After the child is reaped, so the writer's pipe is closed and its thread
    // cannot still be blocked on a full one.
    let _ = writer.join();
    let out = out.join().unwrap_or_default();
    let err = err.join().unwrap_or_default();

    match status {
        Ok(status) if status.success() => Verdict {
            id,
            passed: true,
            code: status.code(),
            detail: String::new(),
        },
        Ok(status) => Verdict {
            id,
            passed: false,
            code: status.code(),
            // stderr first: a failing check that bothered to explain itself
            // almost always did it there.
            detail: first_line(if err.trim().is_empty() { &out } else { &err }),
        },
        Err(reason) => Verdict {
            id,
            passed: false,
            code: None,
            detail: reason,
        },
    }
}

/// `std::process::Command` and not `command::blocking`, because this runs in the
/// `warpctrl` process — a CLI that opens no window and owns no job object, so
/// the wrapper's Windows and WSL handling has nothing to do here.
fn shell(run: &str) -> std::process::Command {
    #[cfg(windows)]
    {
        let mut command = std::process::Command::new("cmd");
        command.args(["/C", run]);
        command
    }
    #[cfg(not(windows))]
    {
        let mut command = std::process::Command::new("sh");
        command.args(["-c", run]);
        command
    }
}

fn drain(pipe: Option<&mut impl std::io::Read>) -> String {
    let mut text = String::new();
    if let Some(pipe) = pipe {
        let _ = pipe.read_to_string(&mut text);
    }
    text
}

// ---------------------------------------------------------------------------
// The run record, and the guard over it (`.fork/TASKS.md`, T13.1 — `ZB-PLAN`).
//
// The plan says what should happen. The record says what did. Keeping the
// second is what makes `--resume` possible, and `--resume` is what makes the
// guard mean something: a run that starts from scratch every time can never
// reuse evidence, so nothing an edit does to the plan can invalidate any.
//
// The guard's whole invariant, borrowed as a description rather than as code:
// **a resumed run must never reuse a node's answer that the plan on disk no
// longer justifies.** Two things can break that, and the second is the one you
// would not find by eye:
//
//  1. a finished node's own definition changed — the answer on file was
//     produced by a different prompt, allowlist or set of handoffs;
//  2. a finished node now waits on something that never ran — the plan grew a
//     node upstream of completed work, so a resume would run the new node and
//     then skip the one that was supposed to consume it.
//
// Both are stated relative to *what a resume would reuse*, which is why there
// is no third rule for a deleted node: deleting one changes the `needs` of
// everything downstream, and that is rule 1 on those nodes.
// ---------------------------------------------------------------------------

/// What one run wrote down.
///
/// A `BTreeMap` rather than a `HashMap` because the point of a file is that it
/// diffs: two runs of the same plan should differ only where the run differed.
#[derive(Debug, Default, Serialize, Deserialize)]
pub(super) struct RunRecord {
    /// Names the format for anything that finds the file without being told.
    pub record: String,
    pub version: u32,
    pub nodes: BTreeMap<String, RecordedNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct RecordedNode {
    /// The node as it was when it ran — see [`fingerprint`].
    pub fingerprint: String,
    pub settled: NodeState,
    /// One entry per assertion the node declared, in the order it declared
    /// them. Absent for a node that asserted nothing, which is most of them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verdicts: Vec<Verdict>,
}

/// What is known about a run — during it, and afterwards.
///
/// One type for both directions: [`reusable`] builds one out of a record to say
/// what is already known, and [`run`] returns one to say what is known at the
/// end. They are the same question asked at two times.
#[derive(Debug, Default)]
pub(super) struct Outcome {
    pub states: HashMap<String, NodeState>,
    pub verdicts: HashMap<String, Vec<Verdict>>,
}

const RECORD_KIND: &str = "warpctrl.graph.run";
const RECORD_VERSION: u32 = 1;

/// Where a run writes its record when nobody says otherwise.
///
/// Appended rather than substituted for the extension, so a `plan.toml` and a
/// `plan.json` in the same directory cannot claim the same record.
pub(super) fn default_record_path(plan: &Path) -> PathBuf {
    let mut path = plan.as_os_str().to_owned();
    path.push(".run.json");
    PathBuf::from(path)
}

/// Domain separation, so this hash can never be confused with another one that
/// happens to be built from the same strings.
const FINGERPRINT_DOMAIN: &str = "warpctrl.graph.node.v1";

/// The node, reduced to the parts that decide what its agent was asked to do.
///
/// Everything the runner actually uses is in here and nothing else is:
/// `spawn_params` is `compose_prompt` plus the name and the resolved allowlist,
/// so those are exactly the fields whose change makes a recorded answer
/// unreproducible. `[defaults]` is resolved rather than hashed separately —
/// changing a default changes what a node inheriting it was allowed to do, and
/// a node that names its own is untouched by it.
///
/// Edge *order* counts, because `compose_prompt` appends handoffs in `needs`
/// order and the child reads them in that order.
pub(super) fn fingerprint(plan: &Plan, node: &Node) -> String {
    // A leading `+`/`-` on the optional parts, because otherwise "no allowlist"
    // and an allowlist whose one entry is `-` hash the same. Cheap, and the
    // alternative is a collision nobody would ever debug.
    let mut parts = vec![
        node.id.clone(),
        node.name(),
        node.prompt.trim().to_owned(),
        match node.allow_tools(&plan.defaults) {
            Some(tools) => format!("allow:+{}", tools.join("\u{1f}")),
            None => "allow:-".to_owned(),
        },
    ];
    for need in &node.needs {
        let pass = match need.pass() {
            Some(pass) => format!("+{pass}"),
            None => "-".to_owned(),
        };
        parts.push(format!("need:{}\u{1f}{pass}", need.node()));
    }
    // Assertions are part of what "done" meant. Loosening one is exactly the
    // edit T13.1's guard exists to catch, and it would otherwise be the one
    // edit that left the fingerprint alone.
    for assertion in &node.assertions {
        parts.push(format!(
            "assert:{}\u{1f}{}",
            assertion.id(),
            assertion.run()
        ));
    }

    let mut hasher = Sha256::new();
    hasher.update(FINGERPRINT_DOMAIN.as_bytes());
    for part in &parts {
        hasher.update((part.len() as u64).to_le_bytes());
        hasher.update(part.as_bytes());
    }
    hex::encode(hasher.finalize())
}

/// The ids the record says finished, whether or not the plan still has them.
fn cleared(record: &RunRecord) -> HashSet<&str> {
    record
        .nodes
        .iter()
        .filter(|(_, recorded)| matches!(recorded.settled, NodeState::Done { .. }))
        .map(|(id, _)| id.as_str())
        .collect()
}

/// The nodes a resume would reuse rather than run.
///
/// This is Tusk's `sealed_node_ids` under the fork's shape, and it is smaller
/// than it looks: that design has two node kinds, so a *gate* clears while the
/// work upstream of it is in any state, and the seal has to be an upstream
/// closure. Here every node is its own gate — `ready` refuses to start a node
/// until every edge is `Done` — so a finished node's ancestors are finished by
/// construction, and the closure collapses to the set itself. The closure is
/// still walked, but in [`violations`], where the plan may have grown an
/// ancestor since.
pub(super) fn sealed(plan: &Plan, record: &RunRecord) -> Vec<String> {
    let cleared = cleared(record);
    let mut sealed: Vec<String> = plan
        .nodes
        .iter()
        .filter(|node| cleared.contains(node.id.as_str()))
        .map(|node| node.id.clone())
        .collect();
    sealed.sort();
    sealed
}

/// A place where the plan on disk no longer justifies evidence a resume would
/// reuse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum Violation {
    /// The node finished, and then its definition changed.
    Edited {
        node: String,
        /// Finished nodes that were handed this node's answer. Named because
        /// the edit reaches them too, and they will not be re-run either.
        consumed_by: Vec<String>,
    },
    /// The node finished, and the plan has since put something in front of it
    /// that never ran.
    ReachedBack { node: String, upstream: String },
}

/// Everything wrong with resuming this plan from this record.
///
/// Only meaningful for a plan that has passed [`validate`]: the upstream walk
/// assumes edges resolve and does not assume they terminate, so it carries a
/// visited set rather than trusting the acyclicity it was promised.
pub(super) fn violations(plan: &Plan, record: &RunRecord) -> Vec<Violation> {
    let cleared = cleared(record);
    let by_id: HashMap<&str, &Node> = plan
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();

    let mut violations = Vec::new();
    for node in &plan.nodes {
        let Some(recorded) = record.nodes.get(&node.id) else {
            continue;
        };
        if !matches!(recorded.settled, NodeState::Done { .. }) {
            continue;
        }

        if recorded.fingerprint != fingerprint(plan, node) {
            violations.push(Violation::Edited {
                node: node.id.clone(),
                consumed_by: consumers(plan, &cleared, &node.id),
            });
            // And nothing more about this node. An un-run node among its *own*
            // `needs` can only have got there by an edit, so the reach-back
            // would be the same edit counted twice. The reach-backs worth
            // printing are the ones below, on nodes nobody touched.
            continue;
        }

        for upstream in uncleared_upstream(&by_id, &cleared, &node.id) {
            violations.push(Violation::ReachedBack {
                node: node.id.clone(),
                upstream,
            });
        }
    }
    violations.sort_by_key(|violation| match violation {
        Violation::Edited { node, .. } => (node.clone(), String::new()),
        Violation::ReachedBack { node, upstream } => (node.clone(), upstream.clone()),
    });
    violations
}

/// The nearest ancestors of `id` that did not finish.
///
/// Walks *through* finished ancestors and stops at the first that is not, for
/// the same reason `newly_blocked` reports the nearest blocker: a plan that
/// grew a chain of three new nodes upstream has one problem, not three, and
/// the chain is readable from the entries the other nodes produce.
fn uncleared_upstream(
    by_id: &HashMap<&str, &Node>,
    cleared: &HashSet<&str>,
    id: &str,
) -> Vec<String> {
    let mut frontier = Vec::new();
    let mut seen: HashSet<&str> = HashSet::from([id]);
    let mut queue: VecDeque<&str> = VecDeque::from([id]);
    while let Some(current) = queue.pop_front() {
        let Some(node) = by_id.get(current) else {
            continue;
        };
        for need in &node.needs {
            let upstream = need.node();
            if !seen.insert(upstream) {
                continue;
            }
            if cleared.contains(upstream) {
                queue.push_back(upstream);
            } else {
                frontier.push(upstream.to_owned());
            }
        }
    }
    frontier.sort();
    frontier
}

/// Finished nodes that were handed `id`'s answer.
///
/// `pass` and not merely `needs`, because an ordering edge carries nothing: a
/// node that only waited on `id` is unaffected by what `id` said.
fn consumers(plan: &Plan, cleared: &HashSet<&str>, id: &str) -> Vec<String> {
    let mut consumers: Vec<String> = plan
        .nodes
        .iter()
        .filter(|node| cleared.contains(node.id.as_str()))
        .filter(|node| {
            node.needs
                .iter()
                .any(|need| need.node() == id && need.pass().is_some())
        })
        .map(|node| node.id.clone())
        .collect();
    consumers.sort();
    consumers
}

/// The states a resume starts from.
///
/// Only `Done` nodes are carried: a node that failed or was skipped produced no
/// answer to reuse, and re-running it is the entire point of resuming. That is
/// also the guard's practical advice — **edit the failure, not the evidence.**
///
/// Fingerprints are not re-checked here. [`violations`] has already refused the
/// run if any disagreed, and checking in two places invites the two checks to
/// drift.
/// Its verdicts come with it, and are **not** re-run.
///
/// A tempting alternative, rejected on purpose: assertions are commands, so
/// re-checking them on every resume would cost nothing and would catch a pass
/// that has since gone stale. But the record is a record *of the run*, and a
/// verdict is part of what happened rather than a live probe — the same reason
/// the node's answer is carried rather than re-derived. If you distrust the
/// record, the command that asks the world again is `graph run` without
/// `--resume`, which re-runs every node and every assertion in it.
pub(super) fn reusable(plan: &Plan, record: &RunRecord) -> Outcome {
    let mut outcome = Outcome::default();
    for node in &plan.nodes {
        let Some(recorded) = record.nodes.get(&node.id) else {
            continue;
        };
        if !matches!(recorded.settled, NodeState::Done { .. }) {
            continue;
        }
        outcome
            .states
            .insert(node.id.clone(), recorded.settled.clone());
        if !recorded.verdicts.is_empty() {
            outcome
                .verdicts
                .insert(node.id.clone(), recorded.verdicts.clone());
        }
    }
    outcome
}

/// The nodes that could start right now.
///
/// Split from the running of them so the scheduling can be asserted without an
/// app, a socket, or a `claude` binary — the same reason `spawn_for` is its own
/// function in `ai::local_agent`.
pub(super) fn ready<'a>(
    plan: &'a Plan,
    states: &HashMap<String, NodeState>,
    running: usize,
    max_parallel: usize,
) -> Vec<&'a Node> {
    let free = max_parallel.saturating_sub(running);
    if free == 0 {
        return Vec::new();
    }
    plan.nodes
        .iter()
        .filter(|node| matches!(states.get(&node.id), Some(NodeState::Pending) | None))
        .filter(|node| {
            node.needs
                .iter()
                .all(|need| matches!(states.get(need.node()), Some(NodeState::Done { .. })))
        })
        .take(free)
        .collect()
}

/// Nodes that can never run now, and the settled node that stopped each.
///
/// Reports the *nearest* blocker rather than the original failure: standing at
/// a skipped node, "what stopped me" is the edge that did not clear, and the
/// chain back to the root cause is readable from the other entries.
pub(super) fn newly_blocked(
    plan: &Plan,
    states: &HashMap<String, NodeState>,
) -> Vec<(String, String)> {
    plan.nodes
        .iter()
        .filter(|node| matches!(states.get(&node.id), Some(NodeState::Pending) | None))
        .filter_map(|node| {
            let blocker = node.needs.iter().find(|need| {
                matches!(
                    states.get(need.node()),
                    Some(
                        NodeState::Failed { .. }
                            | NodeState::Rejected { .. }
                            | NodeState::Skipped { .. }
                    )
                )
            })?;
            Some((node.id.clone(), blocker.node().to_owned()))
        })
        .collect()
}

/// The prompt a node is actually spawned with.
///
/// The handoff is appended rather than substituted into the prompt: a template
/// with a hole in it is a second thing to get right, and an agent reading
/// "Migrate those files" followed by a labelled list does not need one.
pub(super) fn compose_prompt(node: &Node, states: &HashMap<String, NodeState>) -> String {
    let mut prompt = node.prompt.trim().to_owned();
    for need in &node.needs {
        let Some(pass) = need.pass() else { continue };
        let Some(NodeState::Done { output, .. }) = states.get(need.node()) else {
            // Unreachable while `ready` gates on every need being `Done`, and
            // silent rather than fatal if that ever stops being true: a
            // handoff that goes missing should cost context, not the run.
            continue;
        };
        prompt.push_str(&format!(
            "\n\n--- From `{}` ({}):\n{}",
            need.node(),
            pass,
            output.trim()
        ));
    }
    prompt
}

/// The `agent.spawn` parameters for one node.
pub(super) fn spawn_params(
    plan: &Plan,
    node: &Node,
    states: &HashMap<String, NodeState>,
    parent: Option<String>,
) -> AgentSpawnParams {
    AgentSpawnParams {
        prompt: compose_prompt(node, states),
        name: Some(node.name()),
        parent_conversation_id: parent,
        allow_tools: node.allow_tools(&plan.defaults),
    }
}

/// The `agent.read` parameters used to collect a finished node's answer.
///
/// `last: Some(1)` because a node is one prompt and its answer; asking for the
/// whole transcript would hand the next node the handoff it already received,
/// wrapped in its own reply.
pub(super) fn read_params(conversation_id: &str) -> AgentReadParams {
    AgentReadParams {
        conversation_id: conversation_id.to_owned(),
        last: Some(1),
        include_tool_results: false,
    }
}

/// Whether a conversation has finished the turn it was spawned with.
///
/// Both halves are needed and the second is the one that closes a race: a
/// conversation polled in the instant after `agent.spawn` is not busy *yet*,
/// and would otherwise be read as a node that finished immediately with an
/// empty answer. An exchange that exists and has a finish time cannot be that.
pub(super) fn turn_is_finished(is_busy: bool, last_exchange_is_complete: Option<bool>) -> bool {
    !is_busy && last_exchange_is_complete == Some(true)
}

fn invalid(message: impl Into<String>) -> ControlError {
    ControlError::new(ErrorCode::InvalidParams, message)
}

/// How often a running node is asked whether it has finished.
///
/// An agent turn is tens of seconds, so this is far more often than it needs to
/// be. It is cheap — a request over a unix socket to a process on this machine
/// — and the cost of being slow to notice is a whole node's latency added to
/// the critical path.
const POLL: Duration = Duration::from_secs(2);

/// Runs a plan to completion.
///
/// Returns the final state of every node. Errors only for failures of the run
/// *itself* — an unreadable plan, a Warp that cannot be reached. A node that
/// fails is a result, not an error: the other branches of the graph may still
/// be worth running, and the caller needs the whole picture to decide.
///
/// `resumed` seeds nodes that a previous run already finished (see
/// [`reusable`]). Nothing downstream needs to know: a reused node is `Done`
/// before the loop starts, so `ready` will not start it and `compose_prompt`
/// hands its recorded answer along exactly as a fresh one would.
pub(super) fn run(
    plan: &Plan,
    args: &TargetArgs,
    parent: Option<String>,
    max_parallel: usize,
    timeout: Option<Duration>,
    resumed: &Outcome,
    report: &mut dyn FnMut(Event),
) -> Result<Outcome, ControlError> {
    validate(plan)?;

    let by_id: HashMap<&str, &Node> = plan
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let mut states: HashMap<String, NodeState> = plan
        .nodes
        .iter()
        .map(|node| {
            let state = resumed
                .states
                .get(&node.id)
                .cloned()
                .unwrap_or(NodeState::Pending);
            (node.id.clone(), state)
        })
        .collect();
    let mut verdicts = resumed.verdicts.clone();
    for node in &plan.nodes {
        if resumed.states.contains_key(&node.id) {
            report(Event::Reused {
                id: node.id.clone(),
            });
        }
    }
    let mut started_at: HashMap<String, Instant> = HashMap::new();

    loop {
        let running = states
            .values()
            .filter(|state| matches!(state, NodeState::Running { .. }))
            .count();
        for node in ready(plan, &states, running, max_parallel) {
            let params = spawn_params(plan, node, &states, parent.clone());
            let state = match super::commands::send_action(args, ActionKind::AgentSpawn, params) {
                Ok(data) => match serde_json::from_value::<AgentSpawnResult>(data) {
                    Ok(spawned) => {
                        started_at.insert(node.id.clone(), Instant::now());
                        NodeState::Running {
                            conversation_id: spawned.conversation_id,
                        }
                    }
                    Err(error) => NodeState::Failed {
                        conversation_id: String::new(),
                        reason: format!("agent.spawn answered something unreadable: {error}"),
                    },
                },
                // A refused spawn — past the depth cap, an unknown tool name —
                // is this node failing, not the run failing. Its siblings on
                // other branches have nothing to do with it.
                Err(error) => NodeState::Failed {
                    conversation_id: String::new(),
                    reason: error.message.clone(),
                },
            };
            report(Event::Settled {
                id: node.id.clone(),
                state: state.clone(),
            });
            states.insert(node.id.clone(), state);
        }

        // A skip can block a node that blocks another, so this runs to a fixed
        // point rather than once.
        loop {
            let blocked = newly_blocked(plan, &states);
            if blocked.is_empty() {
                break;
            }
            for (id, blocker) in blocked {
                let state = NodeState::Skipped {
                    blocked_by: blocker,
                };
                report(Event::Settled {
                    id: id.clone(),
                    state: state.clone(),
                });
                states.insert(id, state);
            }
        }

        if states.values().all(NodeState::is_settled) {
            return Ok(Outcome { states, verdicts });
        }

        let running: Vec<(String, String)> = states
            .iter()
            .filter_map(|(id, state)| match state {
                NodeState::Running { conversation_id } => {
                    Some((id.clone(), conversation_id.clone()))
                }
                _ => None,
            })
            .collect();
        if running.is_empty() {
            // `validate` rules out the cycle that would cause this, so reaching
            // here means the scheduler and the validator disagree. Saying so is
            // more use than spinning.
            return Err(ControlError::new(
                ErrorCode::Internal,
                "nothing is running and nothing can start, but the plan is not finished",
            ));
        }

        std::thread::sleep(POLL);

        for (id, conversation_id) in running {
            if let Some(timeout) = timeout
                && started_at
                    .get(&id)
                    .is_some_and(|start| start.elapsed() > timeout)
            {
                let state = NodeState::Failed {
                    conversation_id: conversation_id.clone(),
                    reason: format!("still running after {}s", timeout.as_secs()),
                };
                report(Event::Settled {
                    id: id.clone(),
                    state: state.clone(),
                });
                states.insert(id, state);
                continue;
            }

            let data = super::commands::send_action(
                args,
                ActionKind::AgentRead,
                read_params(&conversation_id),
            );
            let read = match data.and_then(|data| {
                serde_json::from_value::<AgentReadResult>(data).map_err(|error| {
                    ControlError::new(
                        ErrorCode::Internal,
                        format!("agent.read answered something unreadable: {error}"),
                    )
                })
            }) {
                Ok(read) => read,
                // A conversation that has gone — closed, or the app restarted —
                // is a node that will never finish. Everything else transient
                // would resolve on the next poll, but there is no way to tell
                // the two apart from here, so this fails the node rather than
                // polling a corpse forever.
                Err(error) => {
                    let state = NodeState::Failed {
                        conversation_id: conversation_id.clone(),
                        reason: error.message.clone(),
                    };
                    report(Event::Settled {
                        id: id.clone(),
                        state: state.clone(),
                    });
                    states.insert(id, state);
                    continue;
                }
            };

            let last_is_complete = read.exchanges.last().map(|exchange| exchange.is_complete);
            if !turn_is_finished(read.conversation.is_busy, last_is_complete) {
                report(Event::Waiting {
                    id: id.clone(),
                    status: read.conversation.status.clone(),
                    blocked_action: read.conversation.blocked_action.clone(),
                });
                continue;
            }

            let state = if read.conversation.status == "success" {
                let output = read
                    .exchanges
                    .last()
                    .and_then(|exchange| exchange.output.clone())
                    .unwrap_or_default();

                // The turn ending `success` means the agent stopped without
                // erroring. Whether the work happened is a separate question,
                // and this is where it gets asked.
                let node = by_id.get(id.as_str()).copied();
                let judged = node.map(|node| evaluate(node, &output)).unwrap_or_default();
                let held = judged.iter().all(|verdict| verdict.passed);
                if !judged.is_empty() {
                    report(Event::Asserted {
                        id: id.clone(),
                        verdicts: judged.clone(),
                    });
                    verdicts.insert(id.clone(), judged);
                }

                if held {
                    NodeState::Done {
                        conversation_id: conversation_id.clone(),
                        output,
                    }
                } else {
                    NodeState::Rejected {
                        conversation_id: conversation_id.clone(),
                        output,
                    }
                }
            } else {
                NodeState::Failed {
                    conversation_id: conversation_id.clone(),
                    reason: format!("the conversation ended `{}`", read.conversation.status),
                }
            };
            report(Event::Settled {
                id: id.clone(),
                state: state.clone(),
            });
            states.insert(id, state);
        }
    }
}

/// `warpctrl graph …`
pub(super) fn run_graph_command(
    command: GraphCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        GraphCommand::Schema => {
            print!("{SCHEMA}");
            Ok(())
        }
        GraphCommand::Check(args) => check(args, output_format),
        GraphCommand::Run(args) => execute(args, output_format),
    }
}

/// The format, written as a plan that runs.
///
/// Documentation that is also an artifact, for the reason every doc example
/// should be: a comment can go stale silently, and this one is parsed by a test
/// and validated by the same `validate` a real plan goes through. If a field is
/// renamed and this is not, the suite says so.
///
/// Written for a reader who is a program. T7.2's premise is an agent turning a
/// milestone into a plan, and an agent that has `warpctrl` should be able to
/// learn the format from `warpctrl` — the alternative is a human pasting
/// documentation into a prompt, which is the human this was supposed to remove.
const SCHEMA: &str = r##"# A warpctrl task graph.
#
#   warpctrl graph check plan.toml            # parse, resolve edges, find cycles
#   warpctrl graph run   plan.toml            # run it, blocking until it settles
#   warpctrl graph run   plan.toml --resume   # ...but skip what already worked
#
# Every node is one `warpctrl agent spawn`: a fresh child agent, in a hidden
# pane, with the prompt below and nothing else. A child does NOT inherit the
# transcript of whatever wrote this plan, so each prompt has to stand alone.
#
# This file is itself a valid plan. `graph schema > plan.toml` is a starting
# point, not just an illustration.

[defaults]
# Inherited by any node that does not name its own. Values are the preset
# `read-only`, or ToolType names: READ_FILES, RUN_SHELL_COMMAND, GREP,
# APPLY_FILE_DIFFS, SUBAGENT, ... Omit the key entirely for no restriction.
#
# Withholding SUBAGENT and RUN_AGENTS is what stops a node spawning children
# of its own.
allow_tools = ["read-only"]

[[node]]
# `id` names this node to the edges below. Unique; keep it short and mechanical.
id = "survey"
# `prompt` is the whole instruction. Say what to produce and in what form —
# a downstream node receives this node's answer verbatim.
prompt = """
List every file under src/ that still calls the old API.
Reply with one path per line and nothing else.
"""

[[node]]
id = "fix"
prompt = """
Migrate the files listed below to the new API.
"""
# A node's own allowlist replaces the default; it does not add to it.
allow_tools = ["read-only", "APPLY_FILE_DIFFS"]
# `needs` is the only edge type, because a dependency IS an edge that carries
# a payload. Two spellings, one concept:
#
#   needs = ["survey"]                                   ordering only
#   needs = [{ node = "survey", pass = "the files" }]     ordering + handoff
#
# With `pass`, the whole of `survey`'s answer is appended to this prompt under
# a heading naming what it is. Without it, nothing is handed along.
needs = [{ node = "survey", pass = "the list of files" }]

# `assert` is what must be TRUE once this node has finished, and it is how a
# node's "done" stops being the agent's own word for it. Each entry is a shell
# command run in the directory you launched `graph run` from, with this node's
# answer on stdin and its id in $WARP_GRAPH_NODE. Non-zero is a failure.
#
#   assert = ["cargo check --quiet"]                       the command names itself
#   assert = [{ id = "compiles", run = "cargo check" }]     ...or give it a label
#
# A node whose assertion fails is `rejected`, not `done`: its dependents are
# skipped and `--resume` will run it again. Prefer asserting about the WORLD
# (does it build, is the old API gone) over the answer, because the answer is
# the claim you are trying to check.
assert = [
  { id = "compiles", run = "cargo check --quiet" },
  { id = "no-old-api", run = "! grep -rq old_api src/" },
]

[[node]]
id = "report"
prompt = """
Write one line saying how many files were migrated.
"""
# Several edges join here. Nodes with no unmet edges run in parallel, bounded
# by --max-parallel (default 4).
needs = [
  { node = "survey", pass = "the original list" },
  { node = "fix", pass = "what was changed" },
]

# Notes that are not fields:
#
# * A node that fails stops the nodes that need it — reported as `skipped`,
#   naming the blocker — and leaves every other branch running. The process
#   exits non-zero if anything did not finish.
# * Five states, and the distinctions are load-bearing. `done` is finished and
#   its assertions held. `rejected` is finished and one did not — re-running it
#   unchanged gives the same thing, so edit the prompt or the assertion.
#   `failed` is the agent erroring, which is usually worth retrying. `skipped`
#   is a node that never started because something it needed did not finish.
# * Nothing is retried. An agent turn is not idempotent.
# * A cycle, an unknown node in `needs`, a duplicate id, an assertion asserted
#   twice, or a misspelled field is refused before anything spawns.
#
# The run record, and what it is for:
#
# * `graph run` writes `plan.toml.run.json` — every settled node, plus a hash of
#   the node as it was when it ran. `--record PATH` moves it, `--no-record`
#   suppresses it. It holds each finished node's answer verbatim, so it is
#   agent-authored text on disk; add it to .gitignore unless you mean to commit
#   it. It is not a transcript — WARP_FORK_EVENT_LOG holds the tool calls.
# * `--resume` carries over every node that finished and re-runs the rest, so a
#   plan whose fourth node failed costs four agent turns to retry, not seven.
#   Pass it every time: with no record, it runs the whole plan.
# * `graph check` picks that record up and refuses a plan the record no longer
#   fits: a finished node whose prompt, allowlist, name or edges changed, or a
#   finished node the plan now runs something new in front of. The advice it is
#   really giving is **edit the failure, not the evidence** — failed and skipped
#   nodes are yours to rewrite freely, because nothing will be reused from them.
"##;

fn load(path: &Path) -> Result<Plan, ControlError> {
    let text = std::fs::read_to_string(path).map_err(|error| {
        ControlError::new(
            ErrorCode::InvalidParams,
            format!("could not read {}: {error}", path.display()),
        )
    })?;
    toml::from_str(&text).map_err(|error| {
        ControlError::new(
            ErrorCode::InvalidParams,
            format!("{} is not a readable plan: {error}", path.display()),
        )
    })
}

/// Reads a run record, if one is meant to be there.
///
/// An explicit `--against` that is missing is an error, because the caller said
/// a file was there. The sibling default going missing is not: a plan that has
/// never run has no record, and that is the ordinary first case.
fn load_record(path: &Path, required: bool) -> Result<Option<RunRecord>, ControlError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !required => return Ok(None),
        Err(error) => {
            return Err(invalid(format!(
                "could not read the run record {}: {error}",
                path.display()
            )));
        }
    };
    let record: RunRecord = serde_json::from_str(&text).map_err(|error| {
        invalid(format!(
            "{} is not a readable run record: {error}",
            path.display()
        ))
    })?;
    if record.record != RECORD_KIND {
        return Err(invalid(format!(
            "{} says it is a `{}`, not a `{RECORD_KIND}`",
            path.display(),
            record.record
        )));
    }
    if record.version != RECORD_VERSION {
        return Err(invalid(format!(
            "{} is a version {} record and this warpctrl writes version {RECORD_VERSION}",
            path.display(),
            record.version
        )));
    }
    Ok(Some(record))
}

/// `graph check` — everything that can be known without spending a token.
fn check(args: GraphCheckArgs, output_format: OutputFormat) -> Result<(), ControlError> {
    let plan = load(&args.plan)?;
    validate(&plan)?;

    // The order is reported in waves rather than as a flat list because the
    // parallelism is the interesting part: a plan whose every node is in its
    // own wave is a plan that will run one agent at a time, and that is usually
    // not what its author drew.
    let waves = waves(&plan);

    let required = args.against.is_some();
    let record_path = args
        .against
        .clone()
        .unwrap_or_else(|| default_record_path(&args.plan));
    let record = load_record(&record_path, required)?;
    let sealed = record
        .as_ref()
        .map(|record| sealed(&plan, record))
        .unwrap_or_default();
    let violations = record
        .as_ref()
        .map(|record| violations(&plan, record))
        .unwrap_or_default();
    let ok = violations.is_empty();

    let summary = serde_json::json!({
        "action": "graph.check",
        "ok": ok,
        "nodes": plan.nodes.len(),
        "waves": waves,
        "record": record.is_some().then(|| record_path.display().to_string()),
        "sealed": sealed,
        "violations": violations,
    });
    match output_format {
        OutputFormat::Json => write_json(&summary)?,
        OutputFormat::Ndjson => write_json_line(&summary)?,
        OutputFormat::Pretty | OutputFormat::Text => {
            println!("{} nodes, {} in sequence", plan.nodes.len(), waves.len());
            for (index, wave) in waves.iter().enumerate() {
                println!("  {}. {}", index + 1, wave.join(", "));
            }
            if record.is_some() {
                println!(
                    "{} sealed by {}{}",
                    sealed.len(),
                    record_path.display(),
                    if sealed.is_empty() {
                        String::new()
                    } else {
                        format!(": {}", sealed.join(", "))
                    }
                );
            }
            for violation in &violations {
                println!("  {}", render_violation(violation));
            }
        }
    }

    if ok {
        Ok(())
    } else {
        // The same channel `graph run` uses for a plan's own verdict: the check
        // worked, and its answer is no.
        Err(ControlError::new(
            ErrorCode::TargetStateConflict,
            "the plan no longer justifies work the run record says is finished; \
             see the entries above, or delete the record to start over",
        ))
    }
}

fn render_violation(violation: &Violation) -> String {
    match violation {
        Violation::Edited { node, consumed_by } => {
            let mut line = format!("`{node}` finished, and then its definition changed");
            if !consumed_by.is_empty() {
                line.push_str(&format!(
                    " — its answer was handed to {}, which will not run again either",
                    consumed_by
                        .iter()
                        .map(|id| format!("`{id}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            line
        }
        Violation::ReachedBack { node, upstream } => format!(
            "`{node}` finished, but the plan now runs `{upstream}` before it, and `{upstream}` never did"
        ),
    }
}

/// The nodes grouped by how many edges deep they are.
///
/// Only meaningful for a validated plan: a cycle would never drain, so this
/// stops when it stops making progress rather than looping forever.
fn waves(plan: &Plan) -> Vec<Vec<String>> {
    let mut done: HashSet<&str> = HashSet::new();
    let mut waves = Vec::new();
    while done.len() < plan.nodes.len() {
        let mut wave: Vec<String> = plan
            .nodes
            .iter()
            .filter(|node| !done.contains(node.id.as_str()))
            .filter(|node| node.needs.iter().all(|need| done.contains(need.node())))
            .map(|node| node.id.clone())
            .collect();
        if wave.is_empty() {
            break;
        }
        wave.sort();
        for id in &wave {
            done.insert(
                plan.nodes
                    .iter()
                    .find(|node| &node.id == id)
                    .map(|node| node.id.as_str())
                    .unwrap_or_default(),
            );
        }
        waves.push(wave);
    }
    waves
}

/// `graph run` — the plan, actually run.
fn execute(args: GraphRunArgs, output_format: OutputFormat) -> Result<(), ControlError> {
    let plan = load(&args.plan)?;
    validate(&plan)?;

    let record_path = args
        .record
        .clone()
        .unwrap_or_else(|| default_record_path(&args.plan));

    // Only a resume can reuse anything, so only a resume can have anything
    // invalidated. A run from scratch spawns every node again and is free to
    // ignore whatever the last one wrote down.
    let reuse = if args.resume {
        match load_record(&record_path, false)? {
            Some(record) => {
                let violations = violations(&plan, &record);
                if !violations.is_empty() {
                    let detail = violations
                        .iter()
                        .map(render_violation)
                        .collect::<Vec<_>>()
                        .join("; ");
                    return Err(ControlError::new(
                        ErrorCode::TargetStateConflict,
                        format!(
                            "refusing to resume: {detail}. Run `graph check` for the same list, \
                             fix the plan, or delete {} to run the whole plan again",
                            record_path.display()
                        ),
                    ));
                }
                reusable(&plan, &record)
            }
            // Not an error. `--resume` is meant to be the command you always
            // run, so the first time — when there is nothing to resume from —
            // has to mean "run it all" rather than "you did that wrong".
            None => Outcome::default(),
        }
    } else {
        Outcome::default()
    };

    let mut report = |event: Event| match output_format {
        // Machine formats get every event as it happens. A graph run is minutes
        // long, and a caller that only learns the outcome at the end cannot act
        // on a node that failed in the first thirty seconds.
        OutputFormat::Json | OutputFormat::Ndjson => {
            let _ = write_json_line(&event_json(&event));
        }
        OutputFormat::Pretty | OutputFormat::Text => println!("{}", render(&event)),
    };

    let outcome = run(
        &plan,
        &args.target,
        args.parent,
        args.max_parallel.max(1),
        args.timeout.map(Duration::from_secs),
        &reuse,
        &mut report,
    )?;

    // After the run, and reporting its own failure rather than raising it: the
    // work has already happened, and an exit code that says the plan failed
    // because a file could not be written would be a lie about the plan.
    let recorded = if args.no_record {
        None
    } else {
        match write_record(&record_path, &plan, &outcome) {
            Ok(()) => Some(record_path.display().to_string()),
            Err(error) => {
                eprintln!("warpctrl: {}", error.message);
                None
            }
        }
    };

    let ok = outcome
        .states
        .values()
        .all(|state| matches!(state, NodeState::Done { .. }));
    let summary = serde_json::json!({
        "action": "graph.run",
        "ok": ok,
        "nodes": serde_json::to_value(&outcome.states).unwrap_or_default(),
        "verdicts": serde_json::to_value(&outcome.verdicts).unwrap_or_default(),
        "record": recorded,
    });
    match output_format {
        OutputFormat::Json | OutputFormat::Ndjson => write_json_line(&summary)?,
        OutputFormat::Pretty | OutputFormat::Text => {
            let mut ids: Vec<&String> = outcome.states.keys().collect();
            ids.sort();
            println!("---");
            for id in ids {
                println!(
                    "{}",
                    render(&Event::Settled {
                        id: id.clone(),
                        state: outcome.states[id].clone(),
                    })
                );
                // Which gate said no, under the node that it rejected. Found by
                // running it: the per-assertion lines are printed as they
                // happen, minutes earlier and interleaved with every other
                // node, and the summary is the part anyone actually reads — so
                // without this a rejected node says only that *something*
                // disagreed, which is the fact you already had.
                for verdict in outcome.verdicts.get(id).into_iter().flatten() {
                    if !verdict.passed {
                        let detail = if verdict.detail.is_empty() {
                            String::new()
                        } else {
                            format!(" — {}", verdict.detail)
                        };
                        println!("    assert `{}` FAILED{detail}", verdict.id);
                    }
                }
            }
            if let Some(recorded) = &recorded {
                println!("recorded to {recorded}");
            }
        }
    }

    if ok {
        Ok(())
    } else {
        // Not a `ControlError` about the run — the run worked. This is the
        // plan's own verdict, in the only channel a shell reads.
        Err(ControlError::new(
            ErrorCode::TargetStateConflict,
            "some nodes did not finish; see the per-node states above",
        ))
    }
}

/// The record a finished run leaves behind.
///
/// Every settled node, not only the finished ones: a `failed` entry is what
/// tells the next `--resume` that this node is the one to run again, and a
/// record that held only successes would be indistinguishable from a plan that
/// never had the other nodes in it.
///
/// This does hold each finished node's **answer verbatim**, because that is
/// what a resume hands downstream — so it is agent-authored text on disk, next
/// to the plan, and usually wants a `.gitignore` line. It is not a transcript:
/// the tool calls and the reasoning live in `WARP_FORK_EVENT_LOG` (T11.1), and
/// keeping those roles apart is deliberate.
pub(super) fn build_record(plan: &Plan, outcome: &Outcome) -> RunRecord {
    RunRecord {
        record: RECORD_KIND.to_owned(),
        version: RECORD_VERSION,
        nodes: plan
            .nodes
            .iter()
            .filter_map(|node| {
                let state = outcome.states.get(&node.id)?;
                state.is_settled().then(|| {
                    (
                        node.id.clone(),
                        RecordedNode {
                            fingerprint: fingerprint(plan, node),
                            settled: state.clone(),
                            verdicts: outcome.verdicts.get(&node.id).cloned().unwrap_or_default(),
                        },
                    )
                })
            })
            .collect(),
    }
}

fn write_record(path: &Path, plan: &Plan, outcome: &Outcome) -> Result<(), ControlError> {
    let record = build_record(plan, outcome);
    // Pretty, because the file is meant to be read and diffed by a person as
    // much as by the next run.
    let text = serde_json::to_string_pretty(&record).map_err(|error| {
        ControlError::new(
            ErrorCode::Internal,
            format!("could not serialize the run record: {error}"),
        )
    })?;
    std::fs::write(path, format!("{text}\n")).map_err(|error| {
        ControlError::new(
            ErrorCode::Internal,
            format!(
                "could not write the run record to {}: {error}",
                path.display()
            ),
        )
    })
}

fn event_json(event: &Event) -> serde_json::Value {
    match event {
        Event::Settled { id, state } => serde_json::json!({
            "action": "graph.run",
            "node": id,
            "settled": serde_json::to_value(state).unwrap_or_default(),
        }),
        Event::Waiting {
            id,
            status,
            blocked_action,
        } => serde_json::json!({
            "action": "graph.run",
            "node": id,
            "waiting": status,
            "blocked_action": blocked_action,
        }),
        Event::Reused { id } => serde_json::json!({
            "action": "graph.run",
            "node": id,
            "reused": true,
        }),
        Event::Asserted { id, verdicts } => serde_json::json!({
            "action": "graph.run",
            "node": id,
            "verdicts": verdicts,
        }),
    }
}

fn render(event: &Event) -> String {
    match event {
        Event::Settled { id, state } => match state {
            NodeState::Pending => format!("{id}: pending"),
            NodeState::Running { conversation_id } => format!("{id}: running ({conversation_id})"),
            NodeState::Done { output, .. } => {
                format!("{id}: done — {}", first_line(output))
            }
            NodeState::Failed { reason, .. } => format!("{id}: failed — {reason}"),
            NodeState::Rejected { .. } => {
                format!("{id}: rejected — the turn finished and an assertion did not agree")
            }
            NodeState::Skipped { blocked_by } => {
                format!("{id}: skipped — `{blocked_by}` did not finish")
            }
        },
        Event::Waiting {
            id,
            status,
            blocked_action,
        } => match blocked_action {
            Some(action) => format!("{id}: {status} — waiting on approval for {action}"),
            None => format!("{id}: {status}"),
        },
        Event::Reused { id } => format!("{id}: reused — finished in an earlier run"),
        Event::Asserted { id, verdicts } => verdicts
            .iter()
            .map(|verdict| {
                let mark = if verdict.passed { "ok" } else { "FAILED" };
                let detail = if verdict.detail.is_empty() {
                    String::new()
                } else {
                    format!(" — {}", verdict.detail)
                };
                format!("{id}: assert `{}` {mark}{detail}", verdict.id)
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

/// Enough of an answer to recognise it, on one line.
fn first_line(output: &str) -> String {
    const WIDTH: usize = 72;
    let line = output.trim().lines().next().unwrap_or_default().trim();
    match line.char_indices().nth(WIDTH) {
        Some((cut, _)) => format!("{}…", &line[..cut]),
        None => line.to_owned(),
    }
}

/// What the runner says while it works.
///
/// A graph run is minutes long and mostly silent, and a silent minute is
/// indistinguishable from a hang. `Waiting` exists for that: it carries the
/// conversation's own status, so a node stopped on a permission prompt reads as
/// `blocked` rather than as nothing happening.
pub(super) enum Event {
    Settled {
        id: String,
        state: NodeState,
    },
    Waiting {
        id: String,
        status: String,
        blocked_action: Option<String>,
    },
    /// Carried over from a previous run rather than spawned.
    ///
    /// Announced rather than silent: a resumed run that says nothing about the
    /// nodes it skipped looks like a plan that lost half its work.
    Reused {
        id: String,
    },
    /// What the node's assertions had to say, per assertion.
    ///
    /// Reported even when they all pass, because "the gate ran and agreed" and
    /// "there was no gate" are different facts and the summary alone cannot
    /// tell them apart.
    Asserted {
        id: String,
        verdicts: Vec<Verdict>,
    },
}

#[cfg(test)]
#[path = "graph_tests.rs"]
mod tests;
