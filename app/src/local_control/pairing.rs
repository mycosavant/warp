//! Letting in a device that is not this OS account (T11.4).
//!
//! Everything `warpctrl` authenticates today rests on one fact: the caller
//! proved it is the same Unix user, by connecting to a `0600` socket and being
//! checked against the kernel's reported peer UID. That check is the reason the
//! HTTP endpoint can be as thin as it is, and it is exactly the check a phone
//! on the LAN cannot pass. So a wide bind is not "the same server, listening
//! further" — it is a **second front door**, and it needs its own lock.
//!
//! The lock is three steps, and the shape is deliberate:
//!
//! 1. **A pairing code** — 32 bytes of `OsRng`, good for two minutes, spendable
//!    **once**. Minted only when a local, already-authenticated client asks for
//!    one via `control.pair`. This is what a QR carries.
//! 2. **A device token** — what redeeming the code returns, good for twelve
//!    hours. A paired device holds this and nothing else.
//! 3. **A credential** — minted from a device token exactly as the Unix broker
//!    mints one for a local client, through the same [`super::issue_credential`],
//!    so a paired device is subject to every policy check a local one is.
//!
//! **Why three and not one.** A single long-lived bearer would have to appear in
//! the QR, which means it would also appear wherever the QR was displayed — a
//! terminal scrollback, a screenshot, a photo of a monitor — and stay valid.
//! Splitting them means the only secret that is ever *shown* dies in two minutes
//! and dies the moment it is used, and the only long-lived secret is one that
//! was never displayed anywhere.
//!
//! **And the one the ticket did not list, which the catalog makes mandatory.**
//! `warpctrl`'s catalog contains `input.insert`, `input.submit`, `agent.prompt`,
//! `slash.run` and `remote.wsl.connect`. A pairing path that could mint a
//! credential for any of them would be remote code execution reachable by
//! scanning a QR code — precisely the failure this phase was ordered to avoid.
//! A device token therefore mints from [`pairable_actions`] and nothing else.
//! The local broker keeps the whole catalog, because it has a kernel UID check
//! to justify it; a QR scan is not that.
//!
//! **T11.5 made the argument this file asked for, and it came out asymmetric.**
//! The write half was expected to be one decision. It turned out to be two, with
//! different answers: `agent.deny` presses Escape on an agent that is *already*
//! waiting, so the worst a stolen device token achieves is that something the
//! agent proposed does not happen — that is on the list. `agent.approve` presses
//! Return, which through a CLI agent's own permission prompt is arbitrary code,
//! and no mechanism here makes it safe; it is a choice, so it is one the
//! machine's owner makes with `WARP_FORK_REMOTE_APPROVE` and defaults to no.
//! Splitting one verb into two actions is what made that expressible at all,
//! since what a device holds is a list of *actions*.
use ::local_control::auth::AuthToken;
use ::local_control::{ActionKind, ControlError, ErrorCode};
use chrono::{DateTime, Duration, Utc};

/// What a paired device may obtain a credential for, always.
///
/// The read surface T11.2 built, plus the liveness probe a client needs to tell
/// "the instance went away" from "my network went away", plus (T11.5) the two
/// halves of noticing an agent is stuck and stopping it.
///
/// **This list is the security boundary of the whole feature.** Adding to it is
/// a deliberate act with an argument attached, not a convenience — see the
/// module docs for what is in the catalog next to it. The two T11.5 additions
/// come with theirs:
///
/// * `agent.approvals` is a read, and it reports strictly *less* than
///   `events.subscribe` already does — the stream carries tool names, input
///   previews and working directories for every agent in the instance, which is
///   the same material, live. Withholding the snapshot while granting the stream
///   would only mean a phone that connected late could not see what it had
///   already been entitled to watch arrive.
/// * `agent.deny` is the first *write* on this list, and it earns the place by
///   being monotone: it presses Escape on an agent that Warp's own state says is
///   already waiting, so the most it can cause is that something proposed does
///   not happen. There is no argument in which a stolen device token doing this
///   is worse than the agent proceeding.
/// * `agent.cancel` (T14.21) completes a loop whose *read* half was already
///   granted, and the argument for it needs the criterion stated more carefully
///   than "monotone" — see below, because the loose form admits `kill`.
///
/// **The criterion, said precisely.** The load-bearing test is not "makes less
/// happen" — `kill` and `rm -rf` both pass that. It is that an action can only
/// prevent **proposed future effects** and cannot destroy **existing durable
/// state**. `agent.deny` passes exactly: Escape on a prompt that has not run
/// touches nothing that exists. `window.close` fails it, which is why it is in
/// the refused list rather than here: it discards unsaved panes, and that is
/// state no routine mechanism already destroys.
///
/// `agent.cancel` passes *approximately*, and the delta is written down rather
/// than smoothed over. Cancelling interrupts a turn that may be mid-side-effect
/// — a rebase half-applied, a file half-written — and a hostile token holder
/// watching `events.subscribe`, which is already pairable and already carries
/// tool calls live, could time one to land at the worst moment. What settles it
/// is that interruption is an authority the environment *already* holds over
/// every in-flight operation: Ctrl-C, a lost connection, the stop button in the
/// panel. A tool run that cannot survive being interrupted is already broken by
/// things no token gates. The handler is also Stop and not Kill — the
/// conversation survives, the transcript stays readable, the pane is not
/// discarded — and it emits the same event the panel's own stop button does.
///
/// **Not behind `WARP_FORK_REMOTE_APPROVE`, deliberately.** Cancel is
/// deny-shaped, and gating it beside `agent.approve` would imply the approve
/// argument applies to it. That argument is about *causing* what an agent
/// proposed; this action can only stop it.
///
/// **And it is one-way, which the console must not hide.** Restarting a
/// cancelled turn needs `agent.prompt`, which causes effects and so can never
/// be pairable. A phone can stop a runaway; picking the work back up happens at
/// the machine.
///
/// **What it does not fix**, so it is not sold as a general recovery verb: a
/// CLI agent running in a *pane* is not a conversation, so cancel cannot reach
/// it, and the remedy there is keystrokes into a PTY — which is the boundary
/// this list exists to hold. See T14.20: that limitation is a hook that does
/// not answer yet, not something to tunnel through pairing.
pub(super) const PAIRABLE_ACTIONS: &[ActionKind] = &[
    ActionKind::AppPing,
    ActionKind::AgentList,
    ActionKind::EventsSubscribe,
    ActionKind::AgentApprovals,
    ActionKind::AgentDeny,
    ActionKind::AgentCancel,
];

/// What a paired device may obtain a credential for only when the machine's
/// owner has said so — see [`crate::fork::remote_approve_enabled`].
///
/// `agent.approve` presses Enter on whatever the agent proposed, and through a
/// CLI agent's permission prompt that is arbitrary code. The digest check binds
/// a yes to the exact request it was shown; it does not make that request safe.
/// T11.4's as-built asked that any widening of this list carry an argument, and
/// the honest one is that this widening cannot be made safe by mechanism — only
/// chosen — so it is chosen per machine and defaults to no.
const REMOTE_APPROVE_ACTION: ActionKind = ActionKind::AgentApprove;

/// The pairable set as it stands right now.
///
/// A function rather than a second constant because the answer depends on the
/// environment, and a client asking "what may I do?" should be told what is true
/// for this instance rather than what is true in general.
pub(super) fn pairable_actions() -> Vec<ActionKind> {
    let mut actions = PAIRABLE_ACTIONS.to_vec();
    if crate::fork::remote_approve_enabled() {
        actions.push(REMOTE_APPROVE_ACTION);
    }
    actions
}

/// How long a displayed pairing code stays spendable.
///
/// Short because this is the one secret in the flow that is *shown* to a camera,
/// a scrollback, or whoever else is in the room. Long enough to walk across the
/// room and unlock a phone.
const CODE_LIFETIME: Duration = Duration::minutes(2);

/// How long a paired device stays paired.
///
/// A working day, so a phone paired in the morning is still watching in the
/// evening, and a phone lost on the way home is not.
const DEVICE_LIFETIME: Duration = Duration::hours(12);

/// Ceilings, so a bug cannot grow either list without bound. Both are far above
/// what a person does on purpose: codes are consumed within two minutes, and
/// nobody pairs eight phones.
const MAX_PENDING_CODES: usize = 4;
const MAX_PAIRED_DEVICES: usize = 8;

/// One instance's pairing state, held beside the credential map it feeds.
///
/// Process-local and never written to disk, for the same reason grants are:
/// pairing survives neither a restart nor a copy of the discovery record.
#[derive(Default)]
pub(super) struct Pairings {
    /// Codes minted and not yet spent, newest last.
    ///
    /// A `Vec` rather than a map, and the reason is worth stating because the
    /// obvious reading is wrong. It is **not** that a `HashMap` would leak a
    /// prefix — T11.3 established it would not, since `RandomState` seeds
    /// SipHash per process. It is that there are at most four of these, so a
    /// linear scan costs nothing measurable, and scanning lets the comparison be
    /// [`AuthToken`]'s, which T11.3 made constant-time in the contents. Free is
    /// a good price for one less thing to reason about.
    codes: Vec<PendingCode>,
    /// Device tokens and when each stops working.
    devices: Vec<PairedDevice>,
}

struct PendingCode {
    code: AuthToken,
    expires_at: DateTime<Utc>,
}

struct PairedDevice {
    token: AuthToken,
    expires_at: DateTime<Utc>,
}

/// A minted pairing code and its deadline.
pub(super) struct IssuedCode {
    pub(super) code: AuthToken,
    pub(super) expires_at: DateTime<Utc>,
}

/// A redeemed device token and its deadline.
pub(super) struct IssuedDevice {
    pub(super) token: AuthToken,
    pub(super) expires_at: DateTime<Utc>,
}

impl Pairings {
    /// Mints a single-use code for a local client to display.
    ///
    /// The oldest outstanding code is dropped at the ceiling rather than the
    /// request being refused: asking for a code is how a person retries a
    /// pairing that did not take, and answering that with an error would make
    /// the retry the thing that breaks.
    pub(super) fn issue_code(&mut self, now: DateTime<Utc>) -> IssuedCode {
        self.prune(now);
        if self.codes.len() >= MAX_PENDING_CODES {
            self.codes.remove(0);
        }
        let code = AuthToken::generate();
        let expires_at = now + CODE_LIFETIME;
        self.codes.push(PendingCode {
            code: code.clone(),
            expires_at,
        });
        IssuedCode { code, expires_at }
    }

    /// Spends a pairing code and returns the device token it buys.
    ///
    /// The code is removed whether or not the rest succeeds, which is what makes
    /// it single-use: a code that has been offered once is spent, so a replay
    /// from a photograph of the same QR finds nothing.
    pub(super) fn redeem(
        &mut self,
        offered: &AuthToken,
        now: DateTime<Utc>,
    ) -> Result<IssuedDevice, ControlError> {
        self.prune(now);
        let Some(index) = self
            .codes
            .iter()
            .position(|pending| &pending.code == offered)
        else {
            return Err(ControlError::new(
                ErrorCode::UnauthorizedLocalClient,
                "pairing code is not valid",
            ));
        };
        self.codes.remove(index);
        if self.devices.len() >= MAX_PAIRED_DEVICES {
            self.devices.remove(0);
        }
        let token = AuthToken::generate();
        let expires_at = now + DEVICE_LIFETIME;
        self.devices.push(PairedDevice {
            token: token.clone(),
            expires_at,
        });
        Ok(IssuedDevice { token, expires_at })
    }

    /// Confirms a device token is one this instance issued and has not expired.
    pub(super) fn verify_device(
        &mut self,
        offered: &AuthToken,
        now: DateTime<Utc>,
    ) -> Result<(), ControlError> {
        self.prune(now);
        if self.devices.iter().any(|device| &device.token == offered) {
            return Ok(());
        }
        Err(ControlError::new(
            ErrorCode::UnauthorizedLocalClient,
            "device is not paired with this instance",
        ))
    }

    fn prune(&mut self, now: DateTime<Utc>) {
        self.codes.retain(|pending| pending.expires_at > now);
        self.devices.retain(|device| device.expires_at > now);
    }
}

/// Refuses an action a paired device may not have.
///
/// Stated as an allowlist rather than a denylist on purpose. A denylist is a
/// promise to remember every future catalog addition, and the catalog is 114
/// entries and grows; the failure mode of forgetting is a new action silently
/// becoming remotely reachable.
///
/// (That number was 110 here for four increments while the pinned count moved
/// to 114 — the stale-count hazard `CLAUDE.md` names, in the one comment whose
/// argument depends on the catalog being large and growing. Read it off
/// `catalog_has_exactly_*_retained_actions`, never off prose.)
pub(super) fn ensure_pairable(action: ActionKind) -> Result<(), ControlError> {
    let allowed = pairable_actions();
    if allowed.contains(&action) {
        return Ok(());
    }
    let mut message = format!(
        "{} is not available to a paired device; a paired device may only use: {}",
        action.as_str(),
        allowed
            .iter()
            .map(|action| action.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    // Named rather than left as a flat refusal, because this is the one entry
    // on the list whose absence is a *choice someone made on this machine*, and
    // a client author debugging it would otherwise go looking for a bug.
    if action == REMOTE_APPROVE_ACTION {
        message.push_str(
            ". Saying yes from another device is off unless WARP_FORK_REMOTE_APPROVE is set; \
             agent.deny needs no such switch",
        );
    }
    Err(ControlError::new(
        ErrorCode::InsufficientPermissions,
        message,
    ))
}

/// Builds the URL a QR encodes.
///
/// **The code is in the fragment, and that is not decoration.** A fragment is
/// never sent to the server, so it cannot reach an access log, a proxy, or a
/// `Referer` header — the URL is inert everywhere except the device holding it.
/// Stated honestly: the page that would enforce that boundary does not exist
/// yet, so today this is a convention the pairing client is asked to follow
/// rather than something a browser guarantees. It costs nothing to get right
/// now and would cost a redesign later.
pub(super) fn pair_url(origin: &str, path: &str, code: &AuthToken) -> String {
    format!("http://{origin}{path}#{}", code.secret())
}

#[cfg(test)]
#[path = "pairing_tests.rs"]
mod tests;
