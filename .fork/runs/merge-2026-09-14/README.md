# Upstream merge, 2026-09-14

`HANDOFF-MERGE.md` task 3, by Opus 5. Linux, WSL, no Warp running. Tags:
RAN (a command was run and its output read), READ (read from code or git,
not run), ASSUMED.

| | |
|---|---|
| merge base | `5a6ded1e8`, computed (RAN) |
| upstream | `e3464f102`, 72 commits, fetched just before merging (RAN) |
| merge | `2d7ffdc07`, zero conflicts (RAN) |
| overlap | 47 files, `overlap.txt` (RAN) |
| fixes | `0e6d4323c`, two compile breaks no conflict marked (RAN) |

Pre-merge commits from the same handoff: `1409ce080` (the drift check
reports whether its count was confirmed) and `9750c9bd0` (settings-file
loopback pin, unchecked-`reqwest` pin). Each is calibrated in its body.

## What no conflict marked

A read-only agent went through upstream's side before the compile and
predicted the first break below (READ). The compile found the second (RAN).

1. `3959ea721` added an exhaustive match on `BlocklistAIHistoryEvent`
   (`TerminalView::ai_block_targets_for_history_event`) without the fork's
   `ConversationSettledChanged`. Given `Vec::new()`, matching the file's other
   two no-op arms.
2. `50118d9f1` added `settings_inheritance_scope: &impl TeamScope` to
   `create_hidden_child_agent_conversation`. The fork's warpctrl
   `agent.spawn` path passed three arguments. It now captures
   `team_context_for_operation(ctx)` at spawn, the call upstream's own
   error-child path makes (`child_agent/mod.rs:173`, READ). **Compile-verified
   only; no test drives `agent.spawn`** (RAN, grep).
3. Not code: `a06279712` deleted `crates/command-signatures-v2` from git, but
   ignored build output (`js/.yarn/install-state.gz`, `js/build/main.js`, 24 KB)
   kept the directory, and the `crates/*` workspace glob failed on the missing
   manifest. Moved to `/tmp/warp-merge-leftovers` (RAN). **The Windows clone
   will hit this on its next sync** (ASSUMED, same tree shape).

## Step 5: fork claims that depend on upstream not doing something

From the agent's audit of `git diff 5a6ded1e8 upstream/master` (READ):

- No new `reqwest` client outside tests, no new WebSocket dialler. The new
  `every_reqwest_client_outside_this_crate_is_listed` and the websocket pin
  both pass on the merged tree (RAN), which is the stronger form of the claim.
- No `FeatureFlag` added or removed; every flag `fork.rs` names still exists.
  Four cargo features joined `default` (`restore_prompt_on_inline_model_selector_search`,
  `shell_widget_handoff`, `native_shell_completions`,
  `history_search_ranking_v2`); their call sites are local UI, read not run.
- `crates/ai/src/api_keys.rs` untouched; the validator tests pass (RAN).
- No change under any `local_control` path; both catalog pins pass (RAN).
- Nothing touches `app_version()` or autoupdate.
- **Open, for the maintainer:** upstream now attaches the Factory MCP server
  (`{server_root}/api/v1/mcp/factory`) to Claude Code and Codex runs launched
  through `AgentDriver` (`a48ff8014`, `e3464f102`). Login-gated on both paths,
  and MCP's HTTP transport is one of the unchecked `reqwest` clients, so
  `FactoryMcp` in `FORCE_DISABLED` is the proposal. Not done in the merge.
- **Open:** `window_settings.rs` renames `background_blur_texture` to
  `background_backdrop` with a legacy migration (`e6e1c4e0f`). An existing
  `settings.toml` loading cleanly was not checked.

## Gates

| gate | result |
|---|---|
| `cargo check --workspace --all-targets` | clean after `0e6d4323c`, 4 pre-existing `try_next` deprecation warnings (RAN) |
| `-p egress_policy -p http_client -p websocket -p local_control -p ai --lib` | 8, 15, 28, 43, 385 passed, 0 failed (RAN) |
| catalog pins, pairing pin, `local_sync` byte-stability | pass in both app runs (RAN) |
| `./script/format` | no changes beyond the two fixed files (RAN) |
| `node --check console.js` | ok (RAN) |
| `claude-md-budget.sh` | **fails the local tier** by 62,926 chars, as it did before the merge; CLAUDE.md is not in the merge (RAN) |
| `script/presubmit` | not run; it ends in the budget check above |

### `cargo test -p warp --lib`, twice

| | passed | failed | ignored |
|---|---|---|---|
| run 1 | 7374 | 16 | 11 |
| run 2 | 7373 | 17 | 11 |

15 failed in both; union 18 (`run1-failures.txt`, `run2-failures.txt`).
Against the union of `.fork/runs/lib-baseline-2026-09-12/`'s two files, 23 of
its 38 now pass in both runs, and three are new to the union. Each passes run
alone with `--exact` (RAN), and each existed at the baseline tree `4cd2bff33`:

- `secret_redaction::test::test_detect_secrets_no_regexes_configured`: failed
  in both runs. It reads the global custom-regex state without `#[serial]`,
  while the tests below it write that state under `#[serial]`
  (`secret_redaction_tests.rs:309-318`, READ). Nothing in secret redaction
  changed since the baseline (RAN, `git log`/`git diff -G`). A race in the test
  whose odds the merge's added tests moved, not a merge regression; worth a
  `#[serial]` in its own commit.
- `terminal::view::tests::drag_drop_image_in_cli_agent_long_running_command_pastes_via_clipboard`:
  run 2 only, with `Too many open files (os error 24)` in its log.
- `voice::local_transcriber::tests::a_command_producing_no_stdout_explains_why`:
  run 1 only.

**The baseline record disagrees with its own files.** Its README says 30 in
both runs, 1 only in run 1, 9 only in run 2, union 40. Its files give 30 in
both, 0 only in run 1, 8 only in run 2, union 38 (RAN, `comm`). This record
diffed against the files. The 09-12 record is left as written.

## Not done here

Linux and Windows release rebuilds, the WSL remote-server daemon refresh, the
live smoke (an ACP panel turn, read-aloud, `warpctrl instance list`, and a
`warpctrl agent spawn` for fix 2), and the push.
