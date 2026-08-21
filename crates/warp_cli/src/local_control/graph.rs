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

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::{Duration, Instant};

use local_control::protocol::{
    ActionKind, AgentReadParams, AgentReadResult, AgentSpawnParams, AgentSpawnResult, ControlError,
    ErrorCode,
};
use serde::{Deserialize, Serialize};

use crate::agent::OutputFormat;
use crate::local_control::output::{write_json, write_json_line};
use crate::local_control::{GraphCommand, GraphRunArgs, TargetArgs};

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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
                    Some(NodeState::Failed { .. } | NodeState::Skipped { .. })
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
pub(super) fn run(
    plan: &Plan,
    args: &TargetArgs,
    parent: Option<String>,
    max_parallel: usize,
    timeout: Option<Duration>,
    report: &mut dyn FnMut(Event),
) -> Result<HashMap<String, NodeState>, ControlError> {
    validate(plan)?;

    let mut states: HashMap<String, NodeState> = plan
        .nodes
        .iter()
        .map(|node| (node.id.clone(), NodeState::Pending))
        .collect();
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
            return Ok(states);
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
                NodeState::Done {
                    conversation_id: conversation_id.clone(),
                    output: read
                        .exchanges
                        .last()
                        .and_then(|exchange| exchange.output.clone())
                        .unwrap_or_default(),
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
        GraphCommand::Check(args) => check(&args.plan, output_format),
        GraphCommand::Run(args) => execute(args, output_format),
    }
}

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

/// `graph check` — everything that can be known without spending a token.
fn check(path: &Path, output_format: OutputFormat) -> Result<(), ControlError> {
    let plan = load(path)?;
    validate(&plan)?;

    // The order is reported in waves rather than as a flat list because the
    // parallelism is the interesting part: a plan whose every node is in its
    // own wave is a plan that will run one agent at a time, and that is usually
    // not what its author drew.
    let waves = waves(&plan);
    match output_format {
        OutputFormat::Json => write_json(&serde_json::json!({
            "action": "graph.check",
            "ok": true,
            "nodes": plan.nodes.len(),
            "waves": waves,
        })),
        OutputFormat::Ndjson => write_json_line(&serde_json::json!({
            "action": "graph.check",
            "ok": true,
            "nodes": plan.nodes.len(),
            "waves": waves,
        })),
        OutputFormat::Pretty | OutputFormat::Text => {
            println!("{} nodes, {} in sequence", plan.nodes.len(), waves.len());
            for (index, wave) in waves.iter().enumerate() {
                println!("  {}. {}", index + 1, wave.join(", "));
            }
            Ok(())
        }
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
    let mut report = |event: Event| match output_format {
        // Machine formats get every event as it happens. A graph run is minutes
        // long, and a caller that only learns the outcome at the end cannot act
        // on a node that failed in the first thirty seconds.
        OutputFormat::Json | OutputFormat::Ndjson => {
            let _ = write_json_line(&event_json(&event));
        }
        OutputFormat::Pretty | OutputFormat::Text => println!("{}", render(&event)),
    };

    let states = run(
        &plan,
        &args.target,
        args.parent,
        args.max_parallel.max(1),
        args.timeout.map(Duration::from_secs),
        &mut report,
    )?;

    let ok = states
        .values()
        .all(|state| matches!(state, NodeState::Done { .. }));
    let summary = serde_json::json!({
        "action": "graph.run",
        "ok": ok,
        "nodes": serde_json::to_value(&states).unwrap_or_default(),
    });
    match output_format {
        OutputFormat::Json | OutputFormat::Ndjson => write_json_line(&summary)?,
        OutputFormat::Pretty | OutputFormat::Text => {
            let mut ids: Vec<&String> = states.keys().collect();
            ids.sort();
            println!("---");
            for id in ids {
                println!(
                    "{}",
                    render(&Event::Settled {
                        id: id.clone(),
                        state: states[id].clone(),
                    })
                );
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
}

#[cfg(test)]
#[path = "graph_tests.rs"]
mod tests;
