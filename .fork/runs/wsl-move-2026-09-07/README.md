# WSL move to X: — what broke, why, and the fix

**Date:** 2026-09-07 · **Machine:** Windows 11 **Home** 10.0.26100 · **WSL 2.7.12** ·
distro `Ubuntu` 24.04

> The user's Windows SID is written as `<user-SID>` throughout. It is the only
> value here the repository has never carried before (`git grep -E
> "S-1-5-21-[0-9]"` returns nothing across the tree), and the finding needs the
> *shape* of the path, not the number in it. Usernames and home paths are left
> as-is: they already appear in 132 and 30 tracked files respectively.

## The instruction, and the read behind it was wrong

The guidance followed was:

```
wsl --manage Ubuntu --move X:\wsl\Ubuntu
wsl --manage Ubuntu --resize 2TB
wsl --manage Ubuntu --set-sparse true
```

on the premise that the distro was still on `C:` and needed moving. **It was
already on `X:`.** The Ubuntu Store package folder on `C:` is not a real
directory — it holds seven junctions (`AC`, `AppData`, `LocalCache`,
`LocalState`, `RoamingState`, `Settings`, `TempState`) pointing into
`X:\WpSystem\<user-SID>\AppData\Local\Packages\CanonicalGroupLimited.Ubuntu_79rhkp1fndgsc\`.
That is the layout Settings → Apps → Move leaves behind, and it had been done
previously. The `--move` was unnecessary, and it is what broke WSL.

## The failure

Windows protects relocated Store app data with **app-container EFS** — `cipher
/c` reports compatibility level `Application Protected`, and the directory ACL
carries an app capability SID (`S-1-15-3-…`) with full control. `--move` lifted
`ext4.vhdx` out of that tree into a plain directory. The file kept its
`Encrypted` attribute but left the context where the key is released, so nothing
could open it:

| command | result |
|---|---|
| `--resize 2TB` | `0x80071772` = `ERROR_FILE_ENCRYPTED` |
| `wsl` (disk attach) | `Wsl/Service/CreateInstance/MountDisk/HCS/E_ACCESSDENIED` |
| `File.Open` from the user's own account | Access denied |

The third row is the one that rules out a service-context or ACL explanation.
NTFS permissions were fine throughout (`Authenticated Users:(I)(M)` inherited).

Reproduced on a 5-byte probe before touching the 988 GB file:

```
file created in LocalState : READABLE  (Archive, Encrypted)
same file moved to X:\wsl  : DENIED    (Archive, Encrypted)
same file moved back       : READABLE  (Archive, Encrypted)
```

**`cipher /d` was never an option**: this is Windows Home, the EFS user API
returns "The request is not supported", and the account has no EFS certificate.
Relocation was the only route.

## The fix

1. `Move-Item X:\wsl\Ubuntu\ext4.vhdx` →
   `X:\WpSystem\<user-SID>\…\CanonicalGroupLimited.Ubuntu_79rhkp1fndgsc\LocalState\`.
   Same volume, so an MFT rename: **2.5 ms, no data copied.**
2. `HKCU\Software\Microsoft\Windows\CurrentVersion\Lxss\{d7220a9d-96da-48e0-b54c-ed41f7ed3abe}\BasePath`
   restored from `X:\wsl\Ubuntu` to
   `\\?\C:\Users\onemind\AppData\Local\Packages\CanonicalGroupLimited.Ubuntu_79rhkp1fndgsc\LocalState`.

Verified by booting, not by inspection: `/dev/sdd` mounts at 1007G,
`/home/effatha/git/warp` present, file header reads `vhdxfile`.

**Rule for next time:** never `wsl --manage --move` a Store distro whose app data
has been relocated to another drive. Check for junctions under
`%LOCALAPPDATA%\Packages\<PFN>\` first. The distro is already where you want it.

**`--set-sparse --allow-unsafe` was correctly refused and stays refused.** WSL
disables sparse VHD for data-corruption reasons; not a lever to force under
890 GB of work.

## `.wslconfig` — the section fix was right, plus one addition

`autoMemoryReclaim` belongs under `[experimental]`. It had been under `[wsl2]`
since the file was written on 2026-08-28, where WSL rejects it — every launch
printed `Unknown key 'wsl2.autoMemoryReclaim'`. **So the memory-reclaim
behaviour that file's comment claims has never once been in force**; what has
been holding is `memory=40GB`, `swap=16GB` and the `CARGO_BUILD_JOBS=8` cap.

Also added `swapFile=X:\\wsl\\swap.vhdx`, because WSL defaults swap to
`%LOCALAPPDATA%\Temp` and `C:` was at 4.3% free.

Verified on restart: no `Unknown key` warning, 39Gi memory, 16G swap on
`/dev/sdc`, `X:\wsl\swap.vhdx` created and `%TEMP%\swap.vhdx` absent.

## Unrelated Windows symptoms, correctly attributed

Reported: TTS silent, screen flashes, theme glitching, sticky keys.

Cause is **USB re-enumeration**, not the GPU and not WSL. Every audio device
state change that day falls in one cluster, 14:54:00–14:59:10 (37 events), with
nothing until the user switched the output back at 18:26.
`Kernel-PnP/Configuration` shows a Raspberry Pi Pico arriving as `USBSTOR` and an
Adafruit composite device (`VID_239A`) enumerating repeatedly at 14:44–14:55.
That storm reassigned the default audio endpoint, which is why TTS was playing to
the amp cabinet rather than the PC speakers.

Ruled out with evidence: `llama-server` (started 17:36, an hour and a half later,
and no audio or PnP event follows it); the GPU (no display or PnP event anywhere
near it); the WSL repair (17:55, three hours after the fact).

## Three misreadings made during this session, recorded so they don't propagate

- A `Get-WinEvent -FilterHashtable @{Level=1,2}` query returned **zero** errors
  while the log actually held 6 errors and 25 warnings. False zero from the
  filter shape. Calibrate any log query by first asking for unfiltered counts —
  the same discipline `CLAUDE.md` already demands for `warpctrl agent
  approvals`, where a pretty-format grep reported a phantom zero.
- A theory that GPU HDMI/DP audio endpoints were being re-enumerated was
  advanced and had **no support in any log**. Withdrawn.
- A PowerShell loop reused a stale `$e` when a query errored, printing one
  result twice under two different log names. Re-run before believing repeated
  blocks.

## Disk, and why the resize is closed rather than deferred

The resize was wanted so a 44 GB local model could live in the distro. It is not
needed, on two counts.

**The model stays on the Windows side and is GPU-resident.** 44 GB is its
on-disk size, not its memory footprint. Measured with it loaded: RTX 5070,
12227 MiB total, **11203 MiB in use** — it fits in VRAM with room to spare, and
neither WSL's `memory=40GB` cap nor the vhdx size bears on it.

**And the distro was never short of space; it was full of build artifacts.**

| | before | after |
|---|---|---|
| distro disk | 890G / 94% | **657G / 69%** |
| available | 67G | **299G** |

233G freed by removing 66 `target`/`node_modules`/`build` directories under
`~/git` and `~/dev`. `~/git/warp/target` was deliberately retained (343G).
`~/git/lapce/target` (8.6G) survived because it is owned by `root` from a
`sudo cargo build` in Jul 2025; it needs `sudo rm -rf` and is the only leftover.

On the Windows side, `C:\dev\warp\target` was deleted the same day — 172.5 GB,
taking `C:` from 40.4 GB / 4.3% free to **194.1 GB / 20.9%**. Both warp builds
are therefore cold; the next build on either side is a full one.

**A correction made during the session, recorded because the wrong version was
stated out loud first:** "44 GB model" was read as a RAM figure, and a
conclusion drawn that WSL's `memory=40GB` cap made it impossible. That was an
inference from a number whose units had not been checked. It is a disk figure.
Same shape as the mistakes catalogued above — the reading was fine, the quantity
was assumed.

## Open

- **`--resize` still fails**, with `ERROR_FILE_ENCRYPTED`, even with the vhdx
  back in `LocalState` and attaching cleanly. So the disk attach and the resize
  path do not have the same access to the app-container key. **The mechanism is
  inferred, not measured** — what is confirmed is only that one succeeds and the
  other does not. Left open deliberately: nothing needs it. If it is ever
  needed, the route is to stop depending on the Store app's encrypted folder —
  byte-copy the vhdx (a real content copy, so it lands unencrypted) to a plain
  directory on `X:`, repoint `BasePath`, delete the original. `X:` has ~2.1 TiB
  free, so a ~1 TB copy fits.
- **69 WHEA corrected PCIe errors in 14 days**, all on the root port holding the
  RTX 5070 (`PCI\VEN_8086&DEV_A70D`, bus 0 / dev 1 / fn 0). Corrected, not fatal,
  and not correlated with the symptoms above. Worth a reseat or a Gen4 link
  setting.
