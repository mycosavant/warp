# `WARP_FORK_WINDOW_BOUNDS`, driven live on Windows

2026-09-12, night. The maintainer asked for Warp's window to open at one size
on the main monitor, because a launch left it wherever the platform put it and
both of us then had to find and rearrange windows. `.fork/docs/environment.md`
has the variable's account.

`place.ps1` launches the debug build three times under one scratch profile
(`WARP_DATA_PROFILE=brokerrace`, quit warning off), so each launch restores
what the one before saved. It reads the real window rect with a DPI-aware
`GetWindowRect`, and between launches moves the window with `SetWindowPos`
so the saved position is not the pinned one. `live.txt` is the output,
verbatim. Binary `v0.fork.94d8b32bb-dirty`, the dirt being this commit's three
Rust files.

| launch | variable | saved position going in | opened at |
|---|---|---|---|
| 1 | `1400x900+100+100` | whatever the profile had | **(100,100) 1400x900**, `DISPLAY1`, primary |
| 2 | `1400x900+100+100` | (600,400) 1000x700, moved in launch 1 | **(100,100) 1400x900**, primary |
| 3 | unset | (600,400) 1000x700, moved in launch 2 | (600,400) 1000x700 |

Launch 3 is the control: without the variable, the restored window opens where
it was left, so launch 2's return to the pinned rect is the variable's doing
and not restore's.

The parser test (`window_bounds_take_the_x11_geometry_form_and_refuse_anything_else`)
was calibrated by deleting the leading-`+` guard. The first calibration run
**stayed green**, because none of the refused cases reached the guard:
`++100` fails on an empty X before any number is parsed. `u32::from_str`
accepts `+50`, so only a `+` on Y needs it; `1400x900+100++50` was added,
and the second run failed on exactly that case.

## Not established

- Only one monitor scale, 100%. The logical-to-physical conversion at 125% or
  150% was not exercised, so "logical pixels" is read from
  `crates/warpui/src/windowing/winit/window.rs`, not measured.
- A new window opened from inside Warp (`default_window_options`) was not
  driven; all three launches went through the restored path. The same helper
  wraps both.
- A rect on a second monitor, or one overlapping no monitor, was not tried.
- Linux/WSLg was not run.
- The PowerShell wrapper again did not return after its last line with no
  `warp-oss` left, as in `.fork/runs/broker-race-2026-09-12/`. Two occurrences,
  both with `Start-Process -NoNewWindow`; not investigated.
