# The cloud handoff switched off, measured

2026-09-06, Windows build, the maintainer's own profile, `warpdev.ps1` with
no flags (the product profile). `chip.sh` launches, creates a tab, `cd`s the
pane, lists the slash registry, runs one text-only turn so the agent footer
is drawn, screenshots, closes.

## The first attempt measured the wrong binary

`build.ps1` was run without `-Release`, and `warpdev.ps1` launches
`target\release\warp-oss.exe`. Its own output said so, twice:

```
warpdev: binary C:\dev\warp\target\release\warp-oss.exe
warpdev: the binary predates the tree's last source commit; rebuild with C:\dev\build.ps1 -Release
```

That output went to `launch-attempt1.txt` and was read after the screenshot,
not before. So attempt 1 is the **baseline** on last night's release binary,
with `HandoffLocalCloud` on: `footer-baseline-old-release.png` and
`slash-list-baseline.json`. Kept under those names because a baseline taken
by accident is still a baseline, and this one is what makes attempt 2 a
measurement rather than a confirmation.

## What changed between the two binaries

| instrument | old release (flag on) | `v0.fork.80b4b861b` (forced off) |
|---|---|---|
| the hint row above the agent input | `& send task to the cloud` | `ctrl shift + for code review` |
| `warpctrl slash list` | 64 commands, `/handoff` among them | 63; `/handoff` gone, nothing new |
| the "Hand off to cloud" chip | not drawn | not drawn |

The chip is the surface the ticket named, and it was not in this footer on
either binary; the maintainer's toolbar layout does not include it. The `&`
hint and the registry entry are gated by the same predicate
(`is_cloud_handoff_enabled`, through `is_ampersand_handoff_enabled` and
`static_commands`), so they are the evidence. Note the slash command's
registry name is `/handoff`; `MOVE_TO_CLOUD` is the constant, and the docs
that call it `/move-to-cloud` are naming the constant.

`/cloud-agent` is still in the registry on both. It is `CloudMode` and
`CloudModeFromLocalSession`, a different feature, out of this ticket's scope
and recorded here so nobody reads its presence as this change not landing.

## Files

`chip.sh` (as run for attempt 2), `driver.log`, `launch.txt`,
`footer-release.png`, `slash-list.json`, `tab.json`, `prompt.json`,
`close.json`; attempt 1's `driver-attempt1-wrong-binary.log`,
`launch-attempt1.txt`, `footer-baseline-old-release.png`,
`slash-list-baseline.json`.
