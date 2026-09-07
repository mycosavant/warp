# A model on this machine, 2026-09-07 (T21.5)

The maintainer asked two things: which open-weight model fits the card with
the least loss to quantization, and whether the fork's OpenRouter and local
wiring for the four small AI features works at all. The second question
found a bug before any model ran; it is in `7529749a8` and in
`T03-small-ai-features.md`. This directory is the model half and the
Warp-side run.

## The machine, measured rather than remembered

| | |
|---|---|
| GPU | RTX 5070, 12,227 MiB; **2.3 GB held by the Windows desktop**, so 9.6 GB is what a model gets |
| CPU | i9-14900F, 8 P + 16 E cores, 32 threads |
| RAM | 64 GB DDR5-6400, two sticks; `llmfit` measured ~120 GB/s inside the guest |
| WSL guest | 40 GB (`.wslconfig`), 21 GB available with the usual sessions open |
| disk | **one disk, ~49 GB free.** `C:` reports 49 GB free and the WSL image is on it; the 70 GB `df` shows inside the guest is the image's own headroom, not the host's |
| CUDA | driver 591.86 on both sides; `libcuda` and `nvidia-smi` visible in WSL; **no CUDA toolkit on either side** |
| runtimes present | LM Studio 0.4.21 on Windows, no models downloaded, server not running, `lms.exe` from 2025-05. Nothing in WSL. |

The card is a 5070, not the 5060 the question named; both are 12 GB. The
constraint that actually decides things is the disk, not the VRAM: a
22 GB model is a third of what is free.

## The model field as of 2026-09-07, and why the list in the question is a generation old

Qwen3 (April 2025) has been followed by Qwen 3.5 (Feb 2026, a 9B among
others), Qwen 3.6 (Apr 2026: a 27B dense and a 35B MoE with 3B active) and
Qwen 3.8 (Aug 2026: a 27B dense and a 2.4T flagship, nothing small). Google
shipped Gemma 4 (E2B, E4B, 12B, 26B-A4B, 31B). Z.ai's GLM-4.7-Flash
(Jan 2026) is a 30B-A3B; its successor GLM-5.3-Flash (Aug 2026) is 320B and
not in mainline llama.cpp. Qwen3-Coder-Next (Feb 2026) is 80B-A3B.

Two shapes fit 9.6 GB of VRAM, and they are different bets:

| shape | model, quant, file | where it runs | speed on this class of card | notes |
|---|---|---|---|---|
| dense, all on the GPU | **Gemma 4 12B, UD-Q4_K_XL, 7.4 GB** | VRAM, no RAM | **68 tok/s measured here**, 479 tok/s prompt | multimodal, tool calling first-class, thinking must be switched off for one-shot use |
| | Qwen3.5-9B, UD-Q6_K_XL 8.8 GB or UD-Q4_K_XL 6 GB | VRAM | ~50 tok/s (estimate) | the closest thing to "Qwen3-8B Q8" that exists now; Q8 of a 9B is 10 GB and does not fit beside a KV cache |
| MoE, experts in RAM | **Qwen3.6-35B-A3B, UD-Q4_K_M, 22.1 GB** | ~9 GB VRAM + ~13 GB RAM via `--n-cpu-moe` | ~38 tok/s on an RTX 3060 12 GB at `-ncmoe 24` (InsiderLLM); expect similar here, bound by RAM bandwidth | SWE-bench Verified 73.4; community reports of repeated failed tool calls in agent loops |
| | Gemma 4 26B-A4B, UD-Q4_K_XL ~17 GB | same | 15-22 tok/s reported on a 4060 | 4B active is slower per token than 3B on the CPU side |
| | GLM-4.7-Flash, Q4 ~18 GB | same | untested here | older than Qwen 3.6, a strong agentic score at release |

What does **not** fit at a usable speed: the 27B dense models (Qwen3.6-27B,
Qwen3.8-27B, Gemma 4 31B), 17 GB at Q4. Half the layers land on the CPU and
a dense layer on the CPU runs at RAM speed for every token, so the estimate
is under 10 tok/s; fine for a one-shot commit message, not for an agent
that writes thousands of tokens a turn. And Qwen3-Coder-Next is 46 GB at
4-bit, which this disk cannot hold beside anything else.

On quantization: Unsloth's dynamic 4-bit (`UD-Q4_K_XL`, `UD-Q4_K_M`) is the
standard answer for "least loss at this size" and is what every measurement
above used; Q3 of the MoE (16.8 GB) is the fallback if RAM is tight, at a
measurable cost. `llmfit` 1.1.14, built into the session scratchpad and run
against this machine, ranked the same two families first, though its catalog
is noisy (a 27B tagged as 11.6B, community re-quants ahead of the vendor's).

**The recommendation is two models, one per job.** Gemma 4 12B for the four
small features, because Next Command fires on nearly every prompt and wants
an answer in under a second, and the dense model costs no RAM at all beside
a cargo build. Qwen3.6-35B-A3B for the panel, when the panel runs a local
model, because it is the strongest thing that runs at agent speed here. They
do not fit in VRAM together; llama-server's router mode loads the one asked
for and evicts the other, and a 7 GB load from NVMe is a few seconds.

## The runtime, and why it is llama-server on the Windows side

- **llama.cpp's own server, not Ollama or LM Studio.** Ollama has no expert
  offload flag (its issue 11772 is still open), so the MoE row above cannot
  run there at this VRAM. LM Studio 0.4 replaced the expert-offload toggle
  with a whole-layer slider (its bug 1421). `llama-server` has
  `--n-cpu-moe`, an Anthropic Messages endpoint at `/v1/messages`
  (measured, below), tool calling with `--jinja`, and a router mode that
  holds several models behind one port.
- **Windows side, `C:\dev\llama\b10844\`**, from the official release: the
  release ships a CUDA 13.3 build for Windows and none for Linux, and no
  CUDA toolkit is installed in WSL to build one. No installer; a zip
  unpacked, and `serve.ps1` beside it. Under mirrored networking the Windows
  Warp, a WSL pane and an agent started inside the distribution all reach it
  at `127.0.0.1:8080`; both directions were measured with a throwaway
  listener before anything was downloaded.
- **Models in `X:\models\`** since later the same night, when the maintainer pointed out that C: was nearly full and X: (4 TB, 2.1 TB free) was the drive meant for this; the runtime stays on C:. Gemma 4 12B is there (7.4 GB). The 22 GB
  Qwen is not yet. This line first said the download waited on disk, on a
  reading of the machine that was wrong: X: had 2.1 TB free all along and
  the WSL image was already on it (`../wsl-move-2026-09-07/`).

## The server, measured from WSL

Gemma 4 12B UD-Q4_K_XL, all 99 layers on the GPU, 12,288 tokens of context,
flash attention, `--jinja`. Loads in 4 s. VRAM after loading: 11.2 GB used
of 12.2, 750 MiB free, and Warp's own renderer wants some of that.

| request | answer | tokens | generation | prompt |
|---|---|---|---|---|
| Next Command shape, thinking **on** | **empty `content`**, 64 tokens of `reasoning_content` | 64 | 65 tok/s | 1,176 tok/s |
| the same, thinking off (`LLAMA_ARG_CHAT_TEMPLATE_KWARGS={"enable_thinking":false}`) | `git add crates/ai/src/api_keys.rs` | 14 | 68 tok/s | 479 tok/s for 141 tokens |
| `/v1/messages`, the Anthropic shape `claude-agent-acp` would use | one correct sentence on `git rebase --onto`, `stop_reason: end_turn` | 59 | | |

The first row is the finding: a thinking model behind a `max_tokens: 64`
one-shot returns nothing and no error, and the feature draws a blank. The
fork's client cannot switch thinking off portably (three schemas, three
spellings), so it is the server's job, and `serve.ps1` does it through the
environment because the JSON did not survive `Start-Process`'s re-quoting
on the argument list (the server read `{e` and refused to start).

The first launch attempt is also a trap worth a line: `powershell.exe` run
from WSL does not return while a child it started with redirected output is
alive, so a launch that works looks like a hang. Start it in the background
and poll `/health`.

## The Warp-side run

Windows **debug** build, because a scratch profile on Windows means the
debug binary: `WARP_DATA_PROFILE` is honoured under `cfg!(debug_assertions)`
only, and a release build resolves its directories through the Windows
known-folder API, so a relocated `LOCALAPPDATA` moves the discovery record
and nothing else. Two release attempts each ran on the maintainer's real
profile, never saw the endpoint, and sent the server nothing; the driver
scripts carry the reasons. Profile `localai` is a copy of `wslauto` with the
endpoint above appended to its `settings.toml`; the scratch repository is
`~/scratch-localai`, one commit and two modified files.

| step | binary | what happened | server requests |
|---|---|---|---|
| part 1, first debug build (`7529749a8`) | validator fixed | Next Command and Prompt Suggestions both fired, twice per prompt, and both failed: *"`agents.local_ai.endpoint` is set to "llama", but no Custom Inference endpoints are configured"* | 0 |
| part 1 again (`26c376090`) | reader fixed | **both drew from the local model**: the chip *"What changes were made to main.rs and README.md?"* above the input and `git diff` as ghost text in it, `local-2-settled.png` | 3 |
| part 2, routed pane | | code review panel opened from the footer chip (`ctrl shift +` through `keys.ps1` typed `=`; the panel also opens with `warpctrl surface code-review open`); Commit clicked; **dialog blank**, log: *"Failed to autogenerate commit message: No AI endpoint is configured"* | 0 |
| part 3, the same with `WARP_FORK_WSL_AUTO_CONNECT=0` | | unrouted; Commit clicked; **the message arrived**: *"Update main output and add addition logic / Modify the print statement to include "local model" and add a variable to perform and print a basic addition."*, `local-7-unrouted-commit-dialog.png` | 1 |

Per request, from `server-timings.log`: Next Command 249 prompt tokens, 14
out, 0.4 s; Prompt Suggestions 3,635 prompt tokens (it sends the block
context), 27 out, 2.1 s; the commit message 296 in, 31 out, 0.6 s. Three of
the four features are measured end to end against a model on this machine;
Shared Block Title fires on sharing a block and was not driven.

**The routed gap is the finding of the part 2 and part 3 pair.** In a routed
WSL pane the diff state is `Remote`, and `RemoteDiffStateModel` asks the
daemon inside the distribution to generate the message
(`GitGenerateCommitMessage`), whose handler in `server_model.rs` takes the
daemon's own `AIClient` and calls `generate_code_review_content`. That is
the fork's branch, so `local_completion::config::current()` runs in the
daemon, where `install` never ran and there is no settings file or keychain
to install from, and it answers `NothingConfigured`. The other three
features run in the GUI process and are unaffected. So on the fork's
recommended configuration, a WSL pane auto-routed at bootstrap, the commit
message is the one small feature that is dead, and unrouting the pane is the
workaround. The fix is to have the daemon return the diff and the GUI
generate; that is a protocol addition and is filed, not done.

Two instrument notes. `keys.ps1 -Key Plus -Ctrl -Shift` reached the input
as a bare `=`: the modifier key-downs are posted but the window reads the
modifier state from the keyboard, not from the message stream, so a
shortcut with modifiers does not land through `PostMessage`. And a
`powershell.exe` launched from WSL does not return while a child it started
with redirected output is alive, which made every server launch look like a
hang until it was backgrounded.
