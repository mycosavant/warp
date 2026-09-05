#!/usr/bin/env python3
"""One-shot: lay out `.fork/` by surface. Ran once on 2026-09-05; kept as the record.

What it does, and nothing else:

  * splits `.fork/TASKS.md` at its `## Txx` headings into `tickets/Txx-slug.md`,
    its "Decisions on record" bullets into `decisions/`, and "Open questions"
    into `tickets/open-questions.md`;
  * splits `.fork/IDEAS.md` at its `# Ixx` headings into `tickets/Ixx-slug.md`,
    rewriting the idea board's own `#ixx--…` anchors to file links;
  * `git mv`s the surface pages into `docs/`, the finished history into
    `archive/`, and the run directories into `runs/`;
  * rewrites every `.fork/<old path>` citation in tracked text files to the new
    path. A `.fork/TASKS.md` citation becomes `.fork/tickets/`; the ticket
    number that always follows it in prose is now the file name.

It changes no sentence of prose. Each split file gets one leading line saying
where it came from. The map (`.fork/README.md`) and the CLAUDE.md index are
written by hand afterwards, not here.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(subprocess.check_output(["git", "rev-parse", "--show-toplevel"], text=True).strip())
FORK = ROOT / ".fork"
DATE = "2026-09-05"

TICKET_SLUGS = {
    "T1": "warpctrl",
    "T2": "local-voice",
    "T3": "small-ai-features",
    "T4": "local-drive",
    "T5": "claude-in-oz-seat",
    "T6": "wsl-integration",
    "T7": "work-that-outlives-a-turn",
    "T8": "the-app-you-use",
    "T9": "verifying-the-pixels",
    "T10": "staying-current",
    "T11": "observability-first",
    "T12": "the-console",
    "T13": "run-gate",
    "T14": "acp-adapter",
    "T15": "loose-ends",
    "T16": "wsl-explorer-9p",
    "T17": "lsp-for-agents",
    "T18": "wsl-pane-turns",
    "T19": "agent-footer-chips",
    "T20": "what-run-2-left",
}

IDEA_SLUGS = {
    "I1": "inbox",
    "I2": "composer",
    "I3": "panes-flexible-illegible",
    "I4": "per-pane-zoom",
    "I5": "recent-files-tabs",
    "I6": "follow-the-cwd",
    "I7": "view-as",
    "I8": "visor",
    "I9": "context-is-yours",
    "I10": "browser",
    "I11": "pin-tool-claims",
    "I12": "tooltips",
    "I13": "main-pane-in-group",
    "I14": "context-masking",
    "I15": "computer-use",
    "I16": "wsl-remote-target",
    "I17": "agent-trajectory",
    "I18": "persistent-grant",
    "I19": "agent-ergonomics",
    "I20": "tui-account-gated",
    "I21": "session-openable",
    "I22": "openrouter-provider",
}

MOVES = [
    ("COMPOSER.md", "docs/composer.md"),
    ("VIEWER.md", "docs/observability.md"),
    ("CLASSIFIER.md", "docs/classifier.md"),
    ("README.md", "docs/manual.md"),
    ("SPEC.md", "archive/SPEC.md"),
    ("CONSOLIDATION.md", "archive/CONSOLIDATION.md"),
    ("HANDOFF-CLASSIFIER.md", "archive/HANDOFF-CLASSIFIER.md"),
    ("HANDOFF-COMPOSER.md", "archive/HANDOFF-COMPOSER.md"),
    ("friction-2026-08-31.md", "archive/friction-2026-08-31.md"),
    ("friction-2026-08-31-clean.md", "archive/friction-2026-08-31-clean.md"),
    ("run-2026-09-01", "runs/run-2026-09-01"),
    ("run-2026-09-02", "runs/run-2026-09-02"),
    ("run-selfhost-2026-09-01", "runs/run-selfhost-2026-09-01"),
    ("run-live-2026-09", "runs/run-live-2026-09"),
    ("classifier", "runs/classifier"),
]

# Longest first, so `.fork/README.md` is not eaten by a shorter rule.
CITATION_REWRITES = sorted(
    [(f".fork/{old}", f".fork/{new}") for old, new in MOVES]
    + [
        (".fork/TASKS.md", ".fork/tickets/"),
        (".fork/IDEAS.md", ".fork/tickets/"),
        (".fork/run-*/", ".fork/runs/"),
    ],
    key=lambda p: -len(p[0]),
)

TEXT_SUFFIXES = {".md", ".rs", ".sh", ".py", ".ps1", ".json", ".jsonc", ".jsonl", ".toml", ".txt"}


def git(*args: str) -> None:
    subprocess.check_call(["git", *args], cwd=ROOT)


def pad(ident: str) -> str:
    letter, num = ident[0], int(ident[1:])
    return f"{letter}{num:02d}"


def github_slug(heading: str) -> str:
    s = heading.strip().lower()
    s = re.sub(r"[^\w\s-]", "", s)
    return re.sub(r"\s", "-", s)


def provenance(source: str, what: str) -> str:
    return f"> {what}, split out of `{source}` on {DATE}. History; read the section you came for.\n\n"


def split_at(text: str, level: str) -> list[tuple[str, str]]:
    """Return [(heading_line, body)] with a leading ('', preamble) entry."""
    parts: list[tuple[str, str]] = []
    current_head, buf = "", []
    pat = re.compile(rf"^{re.escape(level)} (?!#)")
    for line in text.splitlines(keepends=True):
        if pat.match(line):
            parts.append((current_head, "".join(buf)))
            current_head, buf = line.rstrip("\n"), []
        else:
            buf.append(line)
    parts.append((current_head, "".join(buf)))
    return parts


def split_tasks() -> None:
    src = FORK / "TASKS.md"
    text = src.read_text()
    parts = split_at(text, "##")
    tickets = FORK / "tickets"
    decisions = FORK / "decisions"
    tickets.mkdir(exist_ok=True)
    decisions.mkdir(exist_ok=True)

    files: dict[Path, str] = {}
    board_head = parts[0][1]  # the preamble, no heading
    board = [board_head]
    for head, body in parts[1:]:
        m = re.match(r"## (T\d+)(\.\d+)?\b", head)
        if m:
            tid = m.group(1)
            path = tickets / f"{pad(tid)}-{TICKET_SLUGS[tid]}.md"
            if path in files:  # `## T4.4 scope`, `## T4.4 as built`
                files[path] += head + "\n" + body
            else:
                files[path] = provenance(".fork/TASKS.md", f"Ticket {tid}") + head + "\n" + body
        elif head.startswith("## Done (carried over"):
            board.append(head + "\n" + body)
        elif head.startswith("## Decisions on record"):
            split_decisions(body, decisions)
        elif head.startswith("## Open questions"):
            files[tickets / "open-questions.md"] = (
                provenance(".fork/TASKS.md", "The open-questions list") + head + "\n" + body
            )
        else:
            sys.exit(f"unplaced TASKS.md section: {head!r}")
    files[tickets / "T00-the-board.md"] = (
        provenance(".fork/TASKS.md", "The board's own preamble and the pre-board checklist")
        + "".join(board)
    )
    for path, content in files.items():
        path.write_text(fix_relative_claude_md(content))
    git("rm", "-q", str(src))


def split_decisions(body: str, out: Path) -> None:
    bullets = re.split(r"\n(?=- \*\*)", body.strip("\n"))
    for b in bullets:
        b = b.strip("\n")
        if not b.startswith("- **"):
            continue
        title = re.match(r"- \*\*(.+?)\*\*", b).group(1)
        date = re.search(r"\((20\d\d-\d\d-\d\d)", b)
        slug = github_slug(re.sub(r"[`:.]", "", title))[:60].rstrip("-")
        name = f"{date.group(1)}-{slug}.md" if date else f"undated-{slug}.md"
        (out / name).write_text(
            f"> A decision on record, split out of `.fork/TASKS.md` on {DATE}. "
            "Binding until a later decision names it.\n\n" + b + "\n"
        )


def split_ideas() -> None:
    src = FORK / "IDEAS.md"
    text = src.read_text()
    parts = split_at(text, "#")
    tickets = FORK / "tickets"
    anchors: dict[str, str] = {}
    tails: dict[str, str] = {}
    files: dict[Path, str] = {}
    board = []
    for head, body in parts:
        m = re.match(r"# (I\d+)\b", head)
        if m:
            iid = m.group(1)
            fname = f"{pad(iid)}-{IDEA_SLUGS[iid]}.md"
            slug = github_slug(head[2:])
            anchors[slug] = fname
            tails[slug.split("--", 1)[1]] = fname
            files[tickets / fname] = provenance(".fork/IDEAS.md", f"Idea {iid}") + head + "\n" + body
        else:
            board.append((head + "\n" if head else "") + body)
    files[tickets / "I00-idea-board.md"] = (
        provenance(".fork/IDEAS.md", "The idea board's preamble and its selection") + "".join(board)
    )

    def relink(m: re.Match) -> str:
        anchor = m.group(1)
        target = anchors.get(anchor) or tails.get(anchor.split("--", 1)[-1])
        if not target:
            sys.exit(f"unresolved idea anchor #{anchor}")
        return f"]({target})"

    for path, content in files.items():
        content = re.sub(r"\]\(#(i\d+--[a-z0-9-]+)\)", relink, content)
        path.write_text(fix_relative_claude_md(content))
    git("rm", "-q", str(src))


def fix_relative_claude_md(text: str) -> str:
    # The boards said `../CLAUDE.md` from `.fork/`; from `.fork/tickets/` that is wrong.
    return text.replace("`../CLAUDE.md`", "`CLAUDE.md`")


def move_files() -> None:
    for old, new in MOVES:
        (FORK / new).parent.mkdir(parents=True, exist_ok=True)
        git("mv", str(FORK / old), str(FORK / new))


def rewrite_citations() -> list[str]:
    changed = []
    me = Path(__file__).resolve()
    tracked = subprocess.check_output(["git", "ls-files", "-z"], cwd=ROOT).decode().split("\0")
    for rel in tracked:
        if not rel:
            continue
        p = ROOT / rel
        if p.suffix not in TEXT_SUFFIXES or not p.is_file() or p.resolve() == me:
            continue
        try:
            text = p.read_text()
        except UnicodeDecodeError:
            continue
        new = text
        for old, repl in CITATION_REWRITES:
            new = new.replace(old, repl)
        if new != text:
            p.write_text(new)
            changed.append(rel)
    return changed


def main() -> None:
    if subprocess.check_output(["git", "status", "--porcelain", "-uno"], cwd=ROOT).strip():
        sys.exit("working tree not clean; commit or stash first")
    split_tasks()
    split_ideas()
    move_files()
    git("add", "-A", str(FORK / "tickets"), str(FORK / "decisions"))
    changed = rewrite_citations()
    print(f"citations rewritten in {len(changed)} files")
    for c in changed:
        print("  ", c)


if __name__ == "__main__":
    main()
