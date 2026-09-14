#!/usr/bin/env python3
"""Install an ACP agent from a lock in this repo, never through a package manager.

    agents.py lock    <name> --package-lock FILE --source URL --commit SHA
                             --entry PATH --node VERSION --node-keyring FILE
    agents.py fetch   <name>     download every locked artifact, verify sha256
    agents.py install <name>     offline: verify again, extract, write bin/<name>
    agents.py path    <name>     print the launch path an install would use

Decided 2026-09-13 (`.fork/decisions/2026-09-13-acp-agents-are-not-launched-through-a-package-manager.md`);
the account is `.fork/docs/agents-supply-chain.md`.

`lock` is the only step that uses npm's output, and it runs when a person bumps
the pin: it reads a `package-lock.json` that was resolved once, downloads each
tarball, checks npm's sha512 integrity, and writes the sha256 of what it got.
The lock is what gets reviewed.

`install` does not import a network module. It extracts tarballs itself, so no
package's lifecycle script can run, because nothing here runs scripts. The
Node runtime is locked too, and only `bin/node` and its LICENSE are extracted:
the installed tree has no `npm` or `npx` in it.

Linux x64 (glibc) only. The Windows launch line runs this install inside WSL;
a native Windows install is not built.
"""

import argparse
import hashlib
import io
import json
import os
import pathlib
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile

try:
    import tomllib
except ImportError:  # Python < 3.11; main() refuses with a message
    tomllib = None

REPO = pathlib.Path(__file__).resolve().parents[2]
LOCKS = REPO / ".fork" / "agents"
PLATFORM = {"os": "linux", "cpu": "x64", "libc": "glibc"}
PLATFORM_NAME = "linux-x64"


def data_home():
    return pathlib.Path(os.environ.get("XDG_DATA_HOME") or pathlib.Path.home() / ".local" / "share")


def cache_home():
    return pathlib.Path(os.environ.get("XDG_CACHE_HOME") or pathlib.Path.home() / ".cache")


def blob_dir():
    return cache_home() / "warp-fork" / "agents" / "blobs"


def die(msg):
    print(f"agents: {msg}", file=sys.stderr)
    sys.exit(1)


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def read_lock(name):
    path = LOCKS / f"{name}.toml"
    if not path.is_file():
        die(f"no lock at {path}")
    with open(path, "rb") as f:
        return tomllib.load(f)


def artifacts(lock):
    """Every file an install needs on this platform, runtime first."""
    out = [a for a in lock["runtime"]["artifact"] if a["platform"] == PLATFORM_NAME]
    if not out:
        die(f"the lock has no {lock['runtime']['name']} runtime for {PLATFORM_NAME}")
    out += [p for p in lock["package"] if p.get("platform", PLATFORM_NAME) == PLATFORM_NAME]
    return out


def prefix(lock):
    a = lock["agent"]
    return data_home() / "warp-fork" / "agents" / a["name"] / a["version"]


# ---- lock ----------------------------------------------------------------------


def download(url):
    import urllib.request  # only `lock` and `fetch` reach the network

    with urllib.request.urlopen(url, timeout=120) as r:
        return r.read()


def matches_platform(entry):
    for key in ("os", "cpu", "libc"):
        listed = entry.get(key)
        if not listed:
            continue
        if all(v.startswith("!") for v in listed):  # npm's blocklist form, `["!win32"]`
            if "!" + PLATFORM[key] in listed:
                return False
        elif PLATFORM[key] not in listed:
            return False
    return True


def toml_str(s):
    """A TOML basic string. `json.dumps` is close but writes characters above
    U+FFFF as surrogate-pair escapes and leaves U+007F raw, both invalid TOML."""
    out = ['"']
    for ch in s:
        cp = ord(ch)
        if ch == '"':
            out.append('\\"')
        elif ch == "\\":
            out.append("\\\\")
        elif cp < 0x20 or cp == 0x7F:
            out.append(f"\\u{cp:04x}")
        else:
            out.append(ch)
    out.append('"')
    return "".join(out)


def cmd_lock(args):
    import base64

    with open(args.package_lock) as f:
        npm_lock = json.load(f)

    keyring = pathlib.Path(args.node_keyring).resolve()
    base = f"https://nodejs.org/dist/{args.node}"
    signed = download(f"{base}/SHASUMS256.txt.asc")
    with tempfile.TemporaryDirectory() as tmp:
        asc = pathlib.Path(tmp) / "SHASUMS256.txt.asc"
        body = pathlib.Path(tmp) / "SHASUMS256.txt"
        asc.write_bytes(signed)
        r = subprocess.run(
            ["gpgv", "--keyring", str(keyring), "--output", str(body), str(asc)],
            capture_output=True, text=True,
        )
        if r.returncode != 0:
            die(f"Node's SHASUMS256.txt signature did not verify:\n{r.stderr}")
        sums = dict(line.split()[::-1] for line in body.read_text().splitlines() if line.strip())
    node_file = f"node-{args.node}-{PLATFORM_NAME}.tar.xz"
    node_sha = sums.get(node_file) or die(f"{node_file} is not in the signed checksums")
    got = hashlib.sha256(download(f"{base}/{node_file}")).hexdigest()
    if got != node_sha:
        die(f"{node_file}: signed sha256 {node_sha}, downloaded {got}")

    root = npm_lock["packages"][f"node_modules/{args.package}"]
    lines = [
        "# Written by `.fork/tools/agents.py lock`. Review the diff; do not hand-edit.",
        "# `.fork/docs/agents-supply-chain.md` says what a bump has to check.",
        "",
        "[agent]",
        f"name = {toml_str(args.name)}",
        f"package = {toml_str(args.package)}",
        f"version = {toml_str(root['version'])}",
        f"source = {toml_str(args.source)}",
        f"commit = {toml_str(args.commit)}",
        f"entry = {toml_str(args.entry)}",
        "",
        "[runtime]",
        'name = "node"',
        f"version = {toml_str(args.node)}",
        "",
        "[[runtime.artifact]]",
        f"platform = {toml_str(PLATFORM_NAME)}",
        f"url = {toml_str(f'{base}/{node_file}')}",
        f"sha256 = {toml_str(node_sha)}",
        f"extract = {toml_str('runtime')}",
        "keep = [\"bin/node\", \"LICENSE\"]",
    ]
    for path, entry in sorted(npm_lock["packages"].items()):
        if not path:
            continue
        platform_specific = any(k in entry for k in ("os", "cpu", "libc"))
        if platform_specific and not matches_platform(entry):
            continue
        if "resolved" not in entry or "integrity" not in entry:
            die(f"{path} has no resolved URL or integrity in the package lock")
        algo, want = entry["integrity"].split("-", 1)
        if algo != "sha512":
            die(f"{path}: integrity is {algo}, expected sha512")
        blob = download(entry["resolved"])
        if base64.b64encode(hashlib.sha512(blob).digest()).decode() != want:
            die(f"{path}: the downloaded tarball does not match npm's sha512 integrity")
        with tarfile.open(fileobj=io.BytesIO(blob)) as t:
            member = next(m for m in t.getmembers() if m.name.split("/", 1)[-1] == "package.json")
            meta = json.load(t.extractfile(member))
        # npm's lock drops `libc`, so a musl binary passes the check above; the
        # tarball's own package.json still carries it.
        if platform_specific and not matches_platform(meta):
            continue
        license_ = meta.get("license")
        if isinstance(license_, dict):
            license_ = license_.get("type")
        name = path.rsplit("node_modules/", 1)[1]
        lines += [
            "",
            "[[package]]",
            f"path = {toml_str(path)}",
            f"name = {toml_str(name)}",
            f"version = {toml_str(entry['version'])}",
            f"license = {toml_str(license_ or 'UNSTATED')}",
            f"url = {toml_str(entry['resolved'])}",
            f"sha256 = {toml_str(hashlib.sha256(blob).hexdigest())}",
            f"integrity = {toml_str(entry['integrity'])}",
            f"extract = {toml_str(path)}",
        ]
        if platform_specific:
            lines.append(f"platform = {toml_str(PLATFORM_NAME)}")
        if meta.get("scripts", {}).keys() & {"preinstall", "install", "postinstall"}:
            lines.append("# has an install script; this installer does not run it")
    text = "\n".join(lines) + "\n"
    try:
        tomllib.loads(text)
    except tomllib.TOMLDecodeError as e:
        die(f"the lock this would write does not parse, so nothing was written: {e}")
    LOCKS.mkdir(parents=True, exist_ok=True)
    out = LOCKS / f"{args.name}.toml"
    out.write_text(text, encoding="utf-8")
    print(f"wrote {out}")


# ---- fetch ---------------------------------------------------------------------


def cmd_fetch(args):
    lock = read_lock(args.name)
    blobs = blob_dir()
    blobs.mkdir(parents=True, exist_ok=True)
    fetched = cached = 0
    for a in artifacts(lock):
        dest = blobs / a["sha256"]
        if dest.is_file() and sha256_file(dest) == a["sha256"]:
            cached += 1
            continue
        data = download(a["url"])
        got = hashlib.sha256(data).hexdigest()
        if got != a["sha256"]:
            die(f"{a['url']}: locked sha256 {a['sha256']}, downloaded {got}. Nothing was written.")
        tmp = dest.with_suffix(".part")
        tmp.write_bytes(data)
        tmp.replace(dest)
        fetched += 1
    print(f"fetched {fetched}, already cached {cached}, into {blobs}")


# ---- install -------------------------------------------------------------------


def extract(blob, dest, keep=None):
    """Extract a tarball with its top directory stripped, refusing unsafe members."""
    with tarfile.open(blob) as t:
        members = []
        for m in t.getmembers():
            parts = m.name.split("/", 1)
            if len(parts) < 2 or not parts[1]:
                continue
            if keep is not None and parts[1] not in keep:
                continue
            m.name = parts[1]
            members.append(m)
        t.extractall(dest, members=members, filter="data")


def cmd_install(args):
    lock = read_lock(args.name)
    target = prefix(lock)
    if target.exists() and not args.force:
        die(f"{target} exists; pass --force to replace it")
    staging = target.with_name(target.name + ".partial")
    if staging.exists():
        make_writable(staging)
        shutil.rmtree(staging)
    staging.mkdir(parents=True)
    try:
        populate(args, lock, target, staging)
    except BaseException:
        make_writable(staging)
        shutil.rmtree(staging, ignore_errors=True)
        raise
    # The old install is removed only once the new one has verified, so a
    # `--force` against a bad cache leaves the working install in place.
    if target.exists():
        make_writable(target)
        shutil.rmtree(target)
    staging.replace(target)
    make_read_only(target)
    print(target / "bin" / args.name)


def populate(args, lock, target, staging):
    blobs = blob_dir()
    for a in artifacts(lock):
        blob = blobs / a["sha256"]
        if not blob.is_file():
            die(f"{a['url']} is not fetched; run `agents.py fetch {args.name}` first")
        got = sha256_file(blob)
        if got != a["sha256"]:
            die(f"{blob}: locked sha256 {a['sha256']}, cached file has {got}. Refusing.")
        extract(blob, staging / a["extract"], keep=a.get("keep"))

    node = staging / "runtime" / "bin" / "node"
    entry = staging / lock["agent"]["entry"]
    if not entry.is_file():
        die(f"the locked entry point {lock['agent']['entry']} is not in the extracted tree")
    bin_dir = staging / "bin"
    bin_dir.mkdir()
    launcher = bin_dir / args.name
    launcher.write_text(
        "#!/bin/sh\n"
        f"# Written by .fork/tools/agents.py install from .fork/agents/{args.name}.toml.\n"
        f'exec "{target / "runtime" / "bin" / "node"}" "{target / lock["agent"]["entry"]}" "$@"\n'
    )
    launcher.chmod(0o755)
    node.chmod(0o755)


def make_read_only(root):
    for dirpath, dirnames, filenames in os.walk(root):
        for f in filenames:
            p = pathlib.Path(dirpath) / f
            if not p.is_symlink():
                p.chmod(p.stat().st_mode & ~(stat.S_IWUSR | stat.S_IWGRP | stat.S_IWOTH))
    for dirpath, dirnames, _ in os.walk(root, topdown=False):
        p = pathlib.Path(dirpath)
        p.chmod(p.stat().st_mode & ~(stat.S_IWUSR | stat.S_IWGRP | stat.S_IWOTH))


def make_writable(root):
    for dirpath, dirnames, filenames in os.walk(root):
        pathlib.Path(dirpath).chmod(0o755)
        for f in filenames:
            p = pathlib.Path(dirpath) / f
            if not p.is_symlink():
                p.chmod(p.stat().st_mode | stat.S_IWUSR)


def cmd_path(args):
    lock = read_lock(args.name)
    print(prefix(lock) / "bin" / args.name)


def main():
    if sys.version_info < (3, 12):
        die("needs Python 3.12 or newer (tomllib, and tarfile's extraction filter)")
    ap =argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)

    lk = sub.add_parser("lock")
    lk.add_argument("name")
    lk.add_argument("--package", required=True, help="the npm package that is the agent")
    lk.add_argument("--package-lock", required=True)
    lk.add_argument("--source", required=True)
    lk.add_argument("--commit", required=True)
    lk.add_argument("--entry", required=True)
    lk.add_argument("--node", required=True, help="e.g. v24.21.0")
    lk.add_argument("--node-keyring", required=True, help="nodejs/release-keys gpg-only-active-keys/pubring.kbx")
    lk.set_defaults(func=cmd_lock)

    for name, func in (("fetch", cmd_fetch), ("path", cmd_path)):
        p = sub.add_parser(name)
        p.add_argument("name")
        p.set_defaults(func=func)

    ins = sub.add_parser("install")
    ins.add_argument("name")
    ins.add_argument("--force", action="store_true")
    ins.set_defaults(func=cmd_install)

    args = ap.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
