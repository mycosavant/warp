# The Windows credential broker race, reproduced, fixed and calibrated

2026-09-12, late evening. Follows the "broker races" trap in
`.fork/runs/focus-live-2026-09-12/README.md`, which saw `warpctrl` fail twice
with `unauthorized_local_client`, `0x80070558`, and a retry succeed.

## The mechanism

`warpctrl` opens the broker pipe and then writes a length-prefixed request
(`crates/local_control/src/client.rs`, `request_credential_over_pipe`). The
broker's connection task called `ensure_same_user_peer`, which starts with
`ImpersonateNamedPipeClient`, before reading anything. Windows refuses that
with `ERROR_CANNOT_IMPERSONATE` until data has been read from the pipe, so the
check passed only when the client's write had already arrived by the time
the task ran.

The fix (`read_authenticated_broker_request`, `app/src/local_control/mod.rs`)
reads the four-byte length prefix, then checks identity, then bounds-checks
and reads the payload. Nothing caller-sent is decoded, and nothing is
allocated from the prefix, before the check. A 5 s timeout now bounds the
read: before, a client that never wrote was refused at once, by accident of
the bug; now it would otherwise hold the task.

## Live, `late-writer.ps1`

Launches one debug Warp under `WARP_DATA_PROFILE=brokerrace`, reads the pipe
name from the discovery record, and sends `{}` after waiting N ms from
connect. `{}` is not a valid request, on purpose: `invalid_request` "failed to
decode" means the peer was authenticated and read; a refusal before the
client writes shows up as "Pipe is broken" on the client's write, because the
broker has already answered and closed.

| binary | wait 0 ms | wait 300 ms | 1000 ms | 3000 ms |
|---|---|---|---|---|
| pre-fix, `v0.fork.bb0af075d` (`live-prefix.txt`) | 2 of 3 authenticated | **0 of 3** | not run | not run |
| post-fix, `v0.fork.3d006bac1-dirty` (`live-postfix.txt`) | 5 of 5 | **3 of 3** | 1 of 1 | 1 of 1 |

The dirty tree was exactly this commit's two Rust files.

The pre-fix run's `window close` did not exit Warp: a quit warning was on
screen and the maintainer confirmed it by hand. The warning appears only with
a running command, a shared session or unsaved code
(`quit_warning/mod.rs:353`); which of the three it was is not established. The
profile's `config/settings.toml` now sets
`general.show_warning_before_quitting = false`, and the post-fix run exited
inside the driver's 30 s wait. The post-fix driver's own PowerShell wrapper
then did not return for over five minutes after printing its last line, with
no `warp-oss` process left; not investigated.

## The unit test, run on Windows

`credential_broker_authenticates_a_client_that_writes_after_connecting`
(`app/src/local_control/mod_tests.rs`), via `test.ps1`:

1. **`test-1-tokio-client.txt`, red, and the test was wrong.** It opened its
   client with tokio's `ClientOptions`, which sets `SECURITY_SQOS_PRESENT` with
   static tracking, so the client's identity was captured at connect and
   impersonating before a read succeeded. The premise assertion failed. The
   fable-reviewer pass had predicted exactly this from the tokio and std
   sources before the run came back.
2. **`test-2-std-client.txt`, green.** Client opened as `warpctrl` opens it,
   `std::fs::OpenOptions` with no QoS flags: impersonating before a read is
   refused, and the late write is authenticated.
3. **`test-3-check-moved-back.txt`, red, the calibration.** With
   `ensure_same_user_peer` moved above the prefix read, the test fails with
   the field error verbatim: "Unable to impersonate using a named pipe until
   data has been read from that pipe. (0x80070558)". File restored after.

Linux `cargo test -p warp --lib local_control`: 187 passed. The change is
`cfg(windows)`; that run shows only that nothing shared broke.

## Not established

- The timeout path was not driven: no client waited past 5 s.
- `warpctrl` itself was not run in a loop against the fixed build; the driver
  imitates its open-then-write order, and the unit test uses its exact open.
- Anonymous- or Identification-level clients, which the reviewer reasoned
  fail closed with a confusing message, before and after, were not run.
- `cargo test -p warp --lib` on Windows beyond this one test was not run.
