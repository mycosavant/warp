# `cargo test -p warp --lib`, whole, twice

> **Superseded in one part, same evening.** The guard below marked "proposed,
> not applied" was applied at the maintainer's request, and widened: `start`,
> `state_of` and `is_active` made the same unguarded lookup, and the footer's
> tick reaches `state_of` after the chip is clicked with fork policy off. All
> four now go through `remote_control::pairing_context`. The 17 pass alone;
> see the commit that follows this record. The body is left as written.

2026-09-12 evening, `HANDOFF-FOCUS.md` Task 3. Tree at `4cd2bff33`; no app
code changed since `aa406e7ca` except two test files. Linux, WSL, no Warp
running, no concurrent build.

| | passed | failed | ignored |
|---|---|---|---|
| run 1 | 7195 | 30 | 11 |
| run 2 | 7187 | 38 | 11 |

Membership, not counts: **30 failed in both**, 1 only in run 1, 9 only in run 2.
The union is 40. Files: `run1-failures.txt`, `run2-failures.txt`.

## The 30 that failed twice, each run alone

Serially as a group (`--test-threads=1`) 29 still failed, but that is not
"alone": `test_insert_into_input` then passed in a three-test run. So each of
the 30 was run in **its own process** through the test binary with `--exact`,
from `app/` as cargo does (`both-runs-each-alone.txt`):

- **7 pass alone.** Order-dependent: they fail only after other tests in the
  same process.
- **23 fail alone.** By first panic line (`alone-failures-by-panic.tsv`):

| count | test or panic | status |
|---|---|---|
| 17 | `Cannot get singleton model of type LocalControlBridge that was never registered` | **new here**, one fork call site, below |
| 1 | `request_usage_model::…::test_byo_api_key_disabled_for_anonymous_firebase_user` | **already known**: an intended fork inversion, A/B'd against `WARP_FORK_POLICY=0` (`.fork/tickets/T03-small-ai-features.md`) |
| 1 | `execution_profiles::…::auth_completion_waits_for_cloud_initial_load_before_migrating` | **already known**, same kind (`.fork/tickets/T04-local-drive.md`; `.fork/docs/build.md` notes it fails serially too) |
| 1 | `workspace::view::…::test_tools_panel_preferences_activate_after_signup_and_ai_enablement` | **already known**, same kind (`T04`) |
| 1 | `server::telemetry::…::test_persist_events_doesnt_include_ugc_events`, `Failed to open file` | not examined |
| 2 | `terminal::input` decorations multibyte, zsh histignorespace, `left == right` | not examined |

## The 17: one fork call site

Backtrace from `terminal::view::tests::agent_view_lifecycle_updates_input_mode`:
`conversation_utils::remove_conversation` → `history_model.rs:2439`
(`remove_conversation_from_memory`) → `local_control::remote_control::stop` →
`LocalControlBridge::as_ref(app)` (`remote_control.rs:48`), which panics when
the singleton is absent.

- `9e496b800` (2026-09-05) wrote `stop` with an unconditional `as_ref`.
- `ae694127a` (2026-09-06, T19) called it, unguarded, on every conversation
  removal and deletion.

Upstream's terminal-view and blocklist fixtures never register the bridge, so
from 2026-09-06 every test that removes a conversation panics.

**It is probably reachable in the app too, and this is read, not run.** The
bridge is registered only when `FeatureFlag::WarpControlCli` is on
(`lib.rs:2639-2645`, plus `feature = "local_fs"` and an App/Test/Tui launch
mode). The fork turns that flag on through `FORCE_ENABLED`, and
`apply_feature_preferences` (`fork.rs:1406`) returns before applying it when
`WARP_FORK_POLICY=0`. The flag is a `DOGFOOD_FLAGS` entry, so policy-off leaves
it off unless the channel is dogfood (not checked for `warp-oss`). So under
the fork's own documented A/B switch, removing a conversation should panic.
The comment above the call says it is *"a no-op under upstream behaviour"*,
which is the configuration where it is not.

Proposed, not applied: guard `stop` with
`if !app.has_singleton_model::<LocalControlBridge>() { return 0; }`. No bridge
means no pairings, so zero devices cut off is the exact answer, not a
degradation. `fork.rs:1010` and `:1232` already guard this way. It touches a
revocation path, so it is left for a decision; calibrate it by watching the 17
go green, and by removing a conversation live under `WARP_FORK_POLICY=0`
before and after.

## Retracted while writing this

The first draft of this file, never committed, said none of these test names
appears in `.fork/`: three do (the rows marked known, found by grepping all 38
distinct names). It also said the app registers the bridge at startup, so a
running Warp was unaffected: the registration is conditional, as above.

## Not established

The 7 order-dependent tests were not traced to what they depend on. The three
unexamined failures were not traced past their panic line. The policy-off crash
was not run. Whether any of the 40 also fail on upstream `master` was not run.
