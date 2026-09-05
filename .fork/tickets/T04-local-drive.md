> Ticket T4, split out of `.fork/TASKS.md` on 2026-09-05. History; read the section you came for.

## T4 — Local-first Warp Drive

Better shape than expected: a full local SQLite store already exists
(`crates/cloud_object_persistence`, diesel + bundled sqlite3). The server is a
**sync layer on top**, with `UpdateSource::{Server, Local}` already
distinguishing origins, plus a working offline mode and `ExportManager`.

So this is "keep the store, neutralize the sync" — not a rewrite.

- [x] **T4.1** Map the server-sync entry points — done, but the premise below
      was wrong. See "The map" and "Three blockers".
- [x] **T4.2** Local-only mode: full read/write, no account, no sync.
      Four seams, one per blocker below plus the guarantee. Not yet exercised
      in a running GUI — see "Not verified".
- [x] **T4.3** Offline read-only banner — answered by T4.1, no work needed.
      It is gated on `NetworkStatus::is_online()` (`drive/index.rs:2439`), not
      on auth, so a logged-out-but-online user never sees it. It is genuinely
      about the network, not the account. Under local-first it becomes a lie
      when the network *does* drop — nothing is read-only then either — so it
      wants suppressing, but that is one condition in T4.2, not its own item.
- [x] **T4.4** Git-backed sync — the mirror is two-way and drivable, it refuses
      to act on a half-merged tree, and workflow aliases travel with their
      workflows. See "T4.4 as built" below.
  - [x] **T4.4a** Lossless object↔file format — `drive/local_sync/format.rs`
  - [x] **T4.4b** Working-tree materializer — `drive/local_sync/tree.rs`
  - [x] **T4.4c** Round trip, replacing T4.5 — three levels of it, below
  - [x] **T4.4d** A trigger — `drive.sync.status` and `drive.sync.export`,
        verified on the Windows build against a real git repository
  - [x] **T4.4e** Conflict policy — both directions refuse a half-merged tree
        rather than reading it as a deletion; see "T4.4e as built" below
  - [x] **T4.4f** Apply an imported tree back into the store — done and
        verified live; see "T4.4f as built" below
  - [x] **T4.4g** Workflow aliases travel inside their workflow's file — found
        by the live run; see "The alias gap" and "T4.4g as built" below
- [x] **T4.7** Deleting a Warp Drive object without an account — the whole
      lifecycle: trash, restore, delete forever, empty trash. See "Two things
      that were never possible without an account" and "T4.7 as built" below.
- [x] **T4.5** Round-trip via the existing import/export paths — **premise is
      wrong, same as T4.1's.** There is no round trip today: export and import
      do not even cover the same set of types, and neither carries identity.
      Replaced by T4.4c, which is done.

Explicitly **not** doing: Proton Drive. No general-purpose public API,
E2E-encrypted with client-side key management; integration means
rclone-shaped reverse engineering, trading a working local store for a
fragile sync target. Revisit only after T4.4 works.

### The map — T4.1 corrected

**There are no server-sync entry points in `cloud_object/model/persistence.rs`.**
That file is 1,838 lines of pure in-memory model: a `HashMap<ObjectUid, Box<dyn
CloudObject>>` plus accessors, with a `SyncSender<ModelEvent>` for SQLite
writes. It never holds a client and never issues a request. `UpdateSource::
{Server, Local}` lives there, but only as a tag on emitted events — nothing
branches on it to decide whether to talk to the network.

The sync layer is `server/cloud_objects/update_manager.rs` (4,833 lines) and
`server/sync_queue.rs` (1,988). Every local write follows the same three steps,
in this order, with no online/offline branch anywhere in them:

    1. update the in-memory CloudModel
    2. save_to_db(...)          -> SQLite, unconditional
    3. SyncQueue::enqueue(...)  -> server, eventually

Step 3 is the only server contact, and it is already decoupled: `enqueue` only
appends. Whether anything is *sent* is one bool, `SyncQueue::should_dequeue`
(`sync_queue.rs:348`), which starts `false` (`:384`).

So the sync is not something the fork has to sever. It is already severed when
logged out, and by exactly one line.

### It already doesn't sync. That is the problem.

`should_dequeue` is set true in exactly one place — `update_manager.rs:1071`,
at the end of `on_changed_objects_fetched`, i.e. only after a server fetch has
*succeeded*. That function is reachable only via `poll_for_updated_objects`,
which early-returns when logged out (`:688`), and polling itself only starts
when `TeamTesterStatus::initiate_data_pollers` fires — emitted from
`auth_manager.rs:449`, on user-fetched.

No account, therefore: no poll, no fetch, no dequeue. Reads and local writes
work; the SQLite store is loaded at startup (`lib.rs:2174`) with no auth check
at all. Nothing leaks.

But the same successful fetch is also the only thing that sets
`UpdateManager::has_initial_load` (`:1073`), and **24 call sites across 15
files `await` that condition** before doing their work. Logged out, they wait
forever. Confirmed consumers include:

    drive/index.rs:961          Warp Drive spinner never stops; sections
                                never initialize (has_initialized_sections)
    ai/agent_sdk/mcp.rs:31       `warp mcp list`
    ai/agent_sdk/profiles.rs:34  `warp profiles list`
    ai/agent_sdk/environment.rs  5 sites
    settings/cloud_preferences_syncer.rs:496, notebooks, env var collections,
    workflow_view, pane_group, workspace/view, docker_sandbox, privacy

**Correction, 2026-08-18, from running the binary.** The two CLI entries above
were previously written up as "never returns *because of this*". That
attribution is wrong. `warp mcp list` does hang forever, but it never reaches
the await: `command_requires_auth` returns `true` for `MCPCommand::List`
(`ai/agent_sdk/mod.rs:1575`), so `launch_command` errors out with "You are not
logged in" first — and then the process hangs anyway, because that error path
never terminates the app. Two separate faults, neither of them blocker 2. The
await is real and the other 22 sites are genuinely blocked by it; these two are
behind an earlier gate. `warp mcp list` reads local Drive objects and arguably
should not need an account at all, but that match arm also covers agent, run,
environment and memory commands which really do talk to Warp's server, so
opening it is its own decision and not part of T4.2.

The drive spinner is gated `show_warp_drive_loading_icon && is_online`
(`index.rs:2515`), so the visible symptom is precisely "logged out but online"
— which is the fork's normal state. Warp Drive looks perpetually loading while
the store underneath it is fully populated and writable.

This inverts the task. "Neutralize the sync" is done. The work is to stop the
app *waiting* for a sync that is never coming.

### Three blockers

1. **No owner.** `UserWorkspaces::personal_drive` (`user_workspaces.rs:979`)
   maps `AuthStateProvider::user_id()` to `Owner::User`, and returns `None`
   when unauthenticated. Every create path needs an `Owner` and every call site
   bails on `None`. So logged out you can read and edit, but cannot create
   anything. One function, ~20 call sites downstream of it — the narrowest
   seam in T4.

2. **`has_initial_load` never fires.** Above. The condition is
   interior-mutable (`reset_initial_load` takes `&self`), so it can be set from
   anywhere. Open question is *when*: at `UpdateManager::new` the SQLite load
   has already happened (`lib.rs:2174` precedes `:2289`), so the state it
   asserts is true — but auth restoration is async, and `auth_manager` only
   calls `reset_initial_load` for `!from_refresh`, so a restored session may
   not re-arm it. Settle this in T4.2 rather than assuming.

3. **Logout deletes the database.** `auth::log_out` (`auth/mod.rs:281`) calls
   `persistence::remove` — "so sessions and cloud objects don't persist between
   accounts" — then `CloudModel::reset()`. Upstream that is safe: the local
   store is a cache of server-owned objects. Once it is authoritative it is
   data loss, and it is reachable from a menu item. This one is not a feature
   gap, it is a hazard, and it did not exist before local-first made the store
   the original rather than the copy.

### T4.2 as built

The local identity is a **fixed sentinel**, `UserUid::new("local")`, not a
per-install UUID. `owner_to_space` files an object under `Space::Personal` only
when its owner equals the *current* user and under `Space::Shared` otherwise, so
a per-machine identity would put a store that moved machines into "Shared with
me" — exactly what T4.4 exists to do. It cannot collide with an account: real
Warp user ids are Firebase uids.

Four seams, all in the established additive style — no upstream behaviour is
deleted, and `WARP_FORK_POLICY=0` restores every one of them:

    fork::local_drive_owner          -> UserWorkspaces::personal_drive
                                        blocker 1: the drive becomes writable
    fork::local_drive_is_authoritative -> UpdateManager::new
                                        blocker 2: the SQLite store *is* the
                                        initial load, so nothing waits forever
                                     -> SyncQueue::enqueue
                                        the guarantee: refused at the door
    fork::local_drive_enabled        -> auth::log_out
                                        blocker 3: the store is not deleted
                                     -> drive::index render_all_sections
                                        T4.3: no false read-only banner

`local_drive_is_authoritative` is the auth-dependent half — fork policy *and*
no account. A fork user who does sign in gets upstream behaviour back, because
their objects then exist somewhere other than this machine.

Two things fell out for free rather than needing work. Objects with pending
changes already render as a laptop icon reading **"Saved locally"** rather than
a spinner: upstream's condition is `has_in_flight_requests &&
!sync_queue_is_dequeueing`, and under local-first the queue never dequeues, so
the correct indicator was already the one that shows. And the Warp Drive
spinner needed no separate fix — it is gated on the same initial-load condition
as everything else, so blocker 2 turned it off.

The enqueue refusal is what turns "does not sync" from an ordering accident
into a property. Upstream already never *sends* while logged out, but the item
survives in the queue, and `lib.rs` reseeds the queue at startup from every
object with pending changes — so the first time an account was added, locally
owned objects would have been pushed under a uid the server has never heard of.
Both paths are now closed, the startup one by owner rather than by auth, since
by then a real account may legitimately be present.

### Verified

- 9 new tests. Each seam is asserted in both directions — logged out *and*
  signed in — because a guard that never turns off would silently break a fork
  user who does log in, and that failure would look like a Warp bug.
- 14 new tests. Full suite **6512 passed / 21 failed** on the final run,
  against a same-session baseline of **6500 / 19** measured by stashing this
  work. Total count rises by exactly 14, matching the new tests. The failure
  delta is the two inversions below plus the flaky set, which varies run to
  run — consecutive runs gave 22, 20 and 21.
- Newly observed in that flaky set:
  `server::cloud_objects::update_manager::tests::
  test_pending_metadata_update_with_polling`. It is in a module this work
  touches, so it was checked rather than assumed: passes 3/3 in isolation and
  under `WARP_FORK_POLICY=0`, and only fails under parallel load. A polling
  timeout, not a regression. Recorded because "it's probably flaky" is exactly
  the reasoning that hides a real fault.
- `cargo clippy -p warp --lib --all-targets` clean; `cargo fmt --check` clean
  for every file touched here.

**Caught by the test suite, and worth recording because it was mine:** the
first version of the `WARP_FORK_POLICY=0` test set and unset the variable
around its assertion. `std::env` is process-wide and the suite runs in
parallel, so it re-enabled fork policy mid-run for whatever happened to be
executing alongside it — and made a `WARP_FORK_POLICY=0` baseline run report
6510/18 instead of the truth. It presented as unrelated tests failing, which is
the expensive kind of wrong. The policy-off path is covered by running the
whole suite with the variable set, which is the real check anyway.

### Verified on Windows, 2026-08-18

Driven from WSL over `powershell.exe`; see `.fork/docs/manual.md` "Driving the
Windows build from WSL" for the mechanics.

    surface warp-drive open   ok: true
    screenshot                Warp Drive renders: PERSONAL space with a `+`,
                              MCP Servers, Rules, TRASH. No spinner.

**Warp Drive renders its contents instead of a perpetual spinner with no
account.** That is the claim T4.2 existed to make, and it holds. The instance
was genuinely account-free — the binary said so itself on another path ("You
are not logged in").

The store path is now known rather than guessed:

    %LOCALAPPDATA%\warp\WarpOss\data\warp.sqlite

- [x] **T4.6** A created object survives a restart — **verified 2026-08-18**,
      and it found a bug. A workflow `simple-workflow-test` was created by hand
      in the GUI with no account (the `+` cannot be reached by scripting; see
      T1.12). In SQLite it is exactly what the design predicts:

          object_type  WORKFLOW
          server_id    <empty>        never synced
          client_id    Client-56cd792e-...
          is_pending   1              -> renders "Saved locally", not a spinner
          subject_uid  local          the sentinel, written by personal_drive

      After a full close-and-relaunch it is still there, editable, with its
      `wf-test` alias intact. The alias lives outside `workflows.data`, which
      is why the payload column does not mention it.

- [x] **T1.12** Add Warp Drive object actions to the local-control catalog.
      Surfaced by T4.2 verification: the catalog can drive every part of the
      app *except* its object store, which makes exactly the fork's own
      headline feature the one thing an agent cannot exercise. 85 actions
      across app, window, tab, pane, session, input, surface, setting, theme,
      appearance, keybinding and file, and nothing that creates a workflow,
      rule or folder. `input.*` writes to the terminal's input editor rather
      than to whatever UI has focus, so the `+` button is unreachable. Same
      shape as the `setting.get/set` allowlist gap recorded under T2.

      Done as four actions — `drive.object.list`, `.get`, `.create`,
      `.trash` — bringing the catalog to **100**. T4.4d's two `drive.sync.*`
      actions were recorded as closing this, and they did close the half of it
      that is "an agent can move the store"; they left the half that is "an
      agent can make one thing". See "T1.12 — as built" below.

### The bug T4.6 caught, and why nothing else could have

The restored workflow came back filed under **"Shared with me"** — an object
this client had created itself one restart earlier.

T4.2 taught `personal_drive` to *write* the local sentinel as owner, but left
`owner_to_space` *reading* `AuthStateProvider::user_id()` directly. Signed in,
those two agree. Account-free they do not: `user_id()` is `None` while the
sentinel is not, so `Some(uid) == None` was false for every locally-created
object and all of them read as somebody else's.

Both sides now resolve through `personal_drive`, which is already the seam that
answers "who am I". For a signed-in user the two forms are identical by
construction, since `personal_drive` is `Owner::User { user_uid: <current> }`.

**Every unit test passed with this bug in place**, and they were not bad tests —
they covered the writing side and the reading side, separately and correctly.
The defect lived in the agreement between them, which is not a place a unit
test naturally looks. It took creating a real object and restarting a real
window. Worth remembering the next time a change looks fully covered: a seam
that is correct at both ends can still be wrong in the middle.

### Known: two T4.2 consequences, surfacing as test failures

`ai::execution_profiles::profiles::tests::
auth_completion_waits_for_cloud_initial_load_before_migrating` fails under fork
policy and passes with `WARP_FORK_POLICY=0` — A/B'd, not assumed. Same category
as the T1 entry above it, and the second such inversion in the fork.

It asserts that legacy execution profiles do not migrate until cloud objects
arrive. Under local-first they migrate at startup instead, because the local
store *is* the load. For a fork user that is the only behaviour that works at
all — waiting for a fetch that never comes means legacy profiles never migrate.
For someone who launches logged out and then signs in, local profiles migrate
first and the server's merge in afterwards via `CloudModelEvent::
InitialLoadCompleted`, which `profiles.rs` already subscribes to. Both sets
survive; the test is asserting the intermediate state, and that state is
genuinely different now.

`workspace::view::tests::
test_tools_panel_preferences_activate_after_signup_and_ai_enablement` fails the
same way and for the same kind of reason, added by the drive-availability fix
below. It asserts the left panel reports `RequiresAccount` for Warp Drive
before signup; under fork policy it reports `Available`, because it is. A/B'd
against `WARP_FORK_POLICY=0`, which passes.

Three inversions in the fork now, all of the same shape: a test pinning
upstream's "this needs an account" premise, which is the premise the fork
exists to remove. Worth watching as a count — if it keeps climbing, the fork is
diverging faster than the seam design intends.

## T4.4 scope — git-backed sync

Scoped 2026-08-18. Not started.

### Most of the machinery is already here

- **`git2` 0.20.4 is already an `app` dependency**, `vendored-libgit2`, so it
  builds without a system libgit2. Already used in three places, including
  `Repository::discover` in `workflows/local_workflows.rs:183`. Like T3 and
  T1.9, this can be done with **zero new dependencies**.
- **Warp already reads workflows out of a git repository.** `WorkflowSource`
  has eight variants, two of which are file-based and have nothing to do with
  Warp Drive: `Project` loads `.warp/workflows/*.yaml` from the discovered git
  worktree, and `Local` loads the user's home workflow directory. So "workflows
  in a git repo" is an upstream feature, not something the fork invents.
- **Live reload already works.** `WarpManagedPathsWatcher` watches the config
  directories and `user_config::native` reloads themes, workflows, launch
  configs and model routers on change. `git pull` is picked up without a
  restart, for free, on whatever paths are wired into it.
- **A complete, type-generic serializer already exists.** `CloudObject::
  serialized() -> SerializedModel` (`cloud_object/mod.rs:497`) is implemented
  for every object type — it is what gets sent to the server and stored. JSON
  types go through `serde_json::to_string` (`json_model.rs:24`).

Useful detail: `load_project_workflows` uses the `WARP_CONFIG_DIR` **constant**
(`.warp`), not `base_warp_config_dir_name()`. So repo workflows live in `.warp`
on every channel, while the *home* config dir is channel-suffixed — `.warp-oss`
for this fork's build. Repo-relative paths are therefore already portable
between the fork and stock Warp; home-relative ones are not.

### What is actually missing

Not git. **A lossless file representation of a cloud object.** The existing
export/import paths are a sharing feature and cannot be reused as a
serialization layer, which is also why T4.5's premise is wrong:

    export  Workflow -> .yaml   Notebook -> .md   EnvVarCollection -> .env
            everything else: `anyhow::bail!("exporting {other:?} not yet supported")`

    import  .md -> Notebook     .yaml/.yml -> Workflow (+ enums)   dirs -> Folders
            nothing else

The two sets are not even the same: export writes env var collections that
import cannot read, and import creates folders that export only expresses as
directory names. Neither carries the object's identity, its folder placement,
its trash state or its timestamps — export serializes `model().data` and
nothing else. So a round trip through them loses the object graph and mints new
ids on the way back in. Ten or so types have no representation at all: AI
facts, MCP servers, templatable MCP servers, execution profiles, workflow
enums, cloud preferences, ambient agent environments, scheduled agents, cloud
agent configs.

The store itself is far more tractable than that suggests. There are only
**four payload shapes** plus one shared spine:

    object_metadata          the spine — type, server_id/client_id, folder_id,
                             trashed_ts, timestamps, creator/editor uids
    object_permissions       owner (subject_uid/subject_type), guests, links
    workflows                data: Text
    notebooks                title, data, ai_document_id
    folders                  name, is_open, is_warp_pack
    generic_string_objects   data: Text   <- all ten JSON types land here

So one file format plus a metadata sidecar covers everything, and the ten
"unsupported" types are collectively a single case.

### Decomposition

- **T4.4a** Lossless object↔file format. The real work. One file per object,
  carrying identity, folder path, and payload. Must survive a round trip
  byte-for-byte on unchanged objects, or every `git status` is dirty.
- **T4.4b** Working-tree materializer: write the whole store to a directory,
  read it back. Folder hierarchy as directories, so the tree is browsable and
  diffs are legible.
- **T4.4c** Round trip, replacing T4.5: materialize → mutate on disk → reload →
  assert the object graph is identical, ids included.
- **T4.4d** Git operations. Thin, given git2 is present.
- **T4.4e** Conflict policy. Cheap or expensive depending on decision 1 below.

### Decisions to settle before starting

1. **Who drives git — Warp, or you?** If Warp auto-commits and pulls, it needs
   a merge and conflict story for a graph of objects with ids, which is a sync
   engine and is where this task's risk actually lives. If Warp only reads and
   writes a directory and *you* run git, conflicts are text conflicts in your
   own repo, T4.4d and T4.4e nearly vanish, and the existing file watcher
   already handles the pull side.
2. **Is the working tree authoritative, or a mirror of SQLite?** T4.2 just made
   SQLite the source of truth. Two sources of truth needs reconciliation; a
   mirror needs a rule for which side wins on divergence.
3. **Does this extend Warp Drive, or the existing `WorkflowSource::Project`
   path?** The second is a much smaller change and already git-native, but it
   is a parallel store — workflows would live in two places, which is the
   confusion upstream already has and the fork would be doubling down on.

Recommendation: user-driven git (1), working tree as a materialized mirror with
SQLite authoritative (2), extending Warp Drive rather than the project path
(3). That keeps the sync engine out of the fork entirely — git is the sync —
and leaves T4.4a as the only substantial piece of work.

**All three settled as recommended, 2026-08-18.**

## T4.4 as built

Three files under `app/src/drive/local_sync/`, 31 tests, zero new
dependencies. The layering is deliberate: `format` and `tree` know nothing
about the app, so their tests are real rather than mock-shaped, and `snapshot`
is the only file that knows about both sides.

    format.rs     one object <-> one file
    tree.rs       one drive <-> one directory
    snapshot.rs   CloudModel  -> the above

### What the format carries, and what it refuses to

A file carries identity, content and content-level metadata. It deliberately
drops:

| dropped | why |
| --- | --- |
| `id`, `shareable_object_id`, `author_id` | per-machine integers, meaningless in another checkout |
| `is_pending`, `retry_count`, `current_editor` | state of a server conversation this fork does not have |
| `folders.is_open` | sidebar view state — expanding a folder would dirty the repo |
| the parent folder id | placement *is* the path, so a move is a rename git can follow |
| `notebooks.conversation_id` | names a conversation on Warp's server; SQLite has no column for it either |

The `is_open` and `folder_id` exclusions are the two that matter. Both are
about the property the whole thing rests on: **an object that has not changed
must produce the bytes it produced last time**, or `git status` is permanently
dirty and the repository is useless as a sync target. `is_open` would break
that on every sidebar click. `folder_id` would not break it, but it would be a
second representation of placement, and two representations of one fact are
two things that can disagree — which is precisely the shape of the bug T4.6
caught.

### Two envelopes, one header

    notebook          <slug>-<hash>.md      YAML front matter + markdown body
    everything else   <slug>-<hash>.json    one JSON object, payload under "data"
    folder            <dir>/.warp-folder.json

Notebooks are prose and belong in a file a diff can read. Everything else is
JSON `serde_json` already produced, and is **re-emitted rather than converted
to YAML**: prettier diffs are not worth a format in which a workflow argument
named `on` or `no` comes back as a boolean. `serde_json`'s maps are `BTreeMap`,
so keys sort and the bytes are stable — pinned by a test, because a workspace
that ever enabled `preserve_order` would silently make byte-stability depend on
hash iteration order.

The filename hash is not decoration. Without it two objects named "deploy"
collide, and disambiguating against siblings would make one object's filename
depend on another's existence — so creating a second "deploy" would rename the
first and churn the repo. The hash makes the name a pure function of the
object.

Ten object types that upstream's export cannot represent at all — AI facts,
MCP servers, execution profiles, cloud preferences, scheduled agents and the
rest — collapse into a single case, because they share one payload column.

### Reading a payload out of a `dyn CloudObject`

There is no accessor for it, and there cannot be a simple one: `CloudObject` is
object-safe and non-generic because `CloudModel` stores its objects as trait
objects, so the model is only reachable by downcasting to the concrete
`GenericCloudObject<K, M>` — thirteen downcasts and a list to maintain by hand.

`update_object_queue_item` is the way through. It is object-safe, it is a pure
constructor that delegates to the model, and every object type has exactly one
`Update*` variant carrying its typed model. One `match` covers all thirteen,
and a new type upstream fails to compile here rather than silently exporting
nothing. Nothing is enqueued — the item is constructed, read and dropped.

### Writing into a directory the user owns

The export target is a repository the user keeps their own things in. An
exporter that treats it as its own is a data-loss bug, not a sync feature, so
the pruning rule is timid: a file is deleted only after it has been read and
**recognised as one this exporter wrote**, a directory only once it is empty,
and dot-directories are never entered. The test for this exports a drive into
a directory holding a README, a `.git`, and the user's own notes, then exports
an *empty* drive over it — the most destructive thing a caller can ask for —
and asserts every one of the user's files is still there.

Trashed objects are exported, with their timestamp. Dropping them would make
an export quietly destructive: emptying the trash is the user's decision, and
an export that pre-empted it would take the undo away.

### Three levels of round trip

Deliberately three, because two correct halves that disagree in the middle is
exactly how T4.6's bug survived a green suite:

1. `format` — one object through one file's bytes and back
2. `tree` — a drive with nested folders through a directory and back
3. `snapshot` — the **live store** through the bridge, onto a disk, and back

Only the third spans the seam between the other two.

### What is left, and why it is the hard half

### T4.4d as built — the trigger

Two actions, `drive.sync.status` and `drive.sync.export`, bringing the catalog
to 87. Namespaced `drive.sync.*` rather than `drive.*` because upstream retired
a whole `drive.*` group and pins the old names as unparseable in
`malformed_and_removed_action_names_are_not_deserialized`; these are not a
revival of those. This also closes T1.12 — none of the other 85 actions touch
the object store.

**The destination is a setting, not a parameter, and that is a security
property rather than a convenience.** An export prunes. If the destination
arrived with the request, anything that could reach local control could aim a
pruning exporter at a directory of its choosing. `warp_drive.local_sync.path`
is also deliberately **not** in `ALLOWLISTED_SETTING_KEYS`, so `setting.set`
cannot repoint it either: an agent can ask for an export but cannot decide
where it lands. Pinned by a test, because adding one line to that allowlist
would undo the whole argument without touching the drive code.

Guards, each naming itself in the error: unset, relative, filesystem root, and
not-a-directory are refused before anything is read or written. A mistyped `/`
would otherwise walk the entire filesystem reading every file to decide whether
it was one of ours. `WARP_FORK_POLICY=0` refuses too — the catalog is a
compile-time list so the action cannot vanish from it, but an action that
deletes files should stop working when its policy is off.

### Verified on Windows, 2026-08-18

Built at 21:09, run against the real store containing the `simple-workflow-test`
workflow created by hand in the previous session:

    drive status     objects 1, path_exists false
    drive export     written 1
    drive export     written 0, unchanged 1     <- the property, live

The file is `simple-workflow-test-8f89f76f.json`, carrying
`uid: Client-56cd792e-...` — the same client id recorded in the SQLite
inspection two sessions ago, so identity really does survive the store → file
boundary.

Then the destructive case, against an actual git repository rather than a
tempdir: `git init` in the mirror, add a README, a `notes.json`, and a
`my-notes/todo.md`, commit, and export twice more.

    removed_files 0        nothing of the user's was touched
    git status --porcelain (empty)      the repository is clean
    .git\HEAD present

That last line is the one worth having. The unit tests assert the same thing
against a tempdir, but "an export leaves a real git repository clean" is the
claim the whole format was designed around, and until now it had only ever been
checked against a directory this code also created.

### T4.4f as built — the mirror is two-way

`drive.sync.import` reads the configured directory into the live store, so a
`git pull` reaches Warp Drive. Thirteen constructors where `snapshot` was
thirteen accessors, but only three bodies: the ten JSON types share a payload
column and therefore share a deserializer.

**An object missing from the tree is trashed, not deleted.** Both alternatives
are wrong, and the reasoning is the load-bearing part of the design:

- *Ignore it* and deletions never propagate. Delete a workflow on machine A,
  pull on B, and B's next export puts the file straight back. The two machines
  resurrect each other's deletions forever.
- *Delete it* and one import against the wrong directory destroys the drive
  with no undo.

Trashing composes with the format instead. A trashed object still exports,
carrying its `trashed` timestamp, so "I deleted this" travels as **content**
rather than as absence. Absence therefore means something stronger — the trash
was emptied — and echoing that as a local trash is the recoverable reading of
it.

The tree wins: no revision comparison, no merge. The moment this starts
deciding which side is newer it is a sync engine, which decision 1 exists to
avoid.

`is_open` is preserved from whatever the machine already had rather than
decided by the import, since it is sidebar state the format deliberately omits.

An empty tree is refused. Pointed at the wrong directory it would read as
"everything was deleted" and trash the whole drive in one call, and a genuinely
empty drive is not distinguishable from a wrong path.

**Consequence worth knowing:** with a single-object drive, deleting that object
and emptying the trash produces an empty tree, which the guard refuses. So the
very last deletion cannot propagate. The safety trade is deliberate, but it is
a real edge and not a theoretical one.

#### Verified on Windows, 2026-08-19

Against the same real store, through the action surface:

    export                     unchanged 1
    (edit the file)
    import                     updated 1
    export                     unchanged 1     <- store and file now agree

    (hand-author a new file)
    import                     created 1
    (delete that file)
    import                     trashed 1
    export                     the object still exports, carrying
                               "trashed": "2026-08-19T03:54:17.595874Z"

    (move the drive files aside)
    import                     invalid_request: refusing to import from a tree
                               with no Warp Drive objects in it

The import also reported the user's own `README.md`, `notes.json` and
`my-notes/todo.md` as ignored, each with the reason — so files that are not
ours are visible rather than silently skipped.

One thing the first attempt at this got wrong, worth recording: copying a file
aside before deleting it produced `unchanged`, not `trashed`. That is correct —
identity is in the header, so a rename is not a delete — but it meant the test
proved nothing until it was redone without the copy.

### Two things that were never possible without an account — T4.7 (both fixed)

Found while designing T4.4f's deletion rule, by reading the path it depends on.

**Trashing — fixed.** `UpdateManager::trash_object` opens with
`let Some(server_id) = id.server_id() else { return; }`. Account-free no object
has a server id, so the Drive panel's Trash item, `WorkflowAction::Trash` and
the workflow modal's delete all silently did nothing. Worse if it had got past
that gate: the local `trashed_ts` is set optimistically and **reverted** when
the request fails, and without credentials it always fails — so there was no
ordering in which the upstream path worked here. Now routed through
`fork::drive_deletes_are_local`, and pinned by a test that was confirmed to
fail with the guard disabled.

**Permanent deletion — fixed, see below.** `UpdateManager::empty_trash` is a
bare server call: it asks `object_client.empty_trash(owner)` and only removes
anything locally on success. Account-free that request cannot succeed, so
emptying the trash did nothing — a trashed object could not be got rid of at
all.

### T4.7 as built — the trash is a place things can leave

Four verbs, not two. The recorded scope was `empty_trash`; reading the path
found the other three, and the last of them is the one that mattered most.

**`empty_trash` and `delete_object_with_initiated_by` are the same bug.** Both
ask the server and only touch anything locally once an answer arrives. The
local half already exists and is already correct — `on_object_delete_success`
does the model, the objects' actions and the SQLite rows — so what is missing
account-free is *only the list of ids the server would have replied with*, and
that list can simply be read: the objects in this space carrying a `trashed_ts`
are what the trash is.

**Descendants have to be walked, not listed.** Trashing a folder marks only the
folder; its contents carry no `trashed_ts` of their own. Delete the trashed set
alone and everything inside a deleted folder is left behind — in memory and in
SQLite — pointing at a parent that no longer exists. The server's reply
includes descendants, which is why upstream never has to think about this.

**The fix had to reach the view, or nothing could have called it.** The Drive
panel gates its trash context menu on `online_only_operation_allowed`, which
requires `has_server_id()`. Account-free that is never true, so "Restore" and
"Delete forever" were never *drawn* on a trashed object. Fixing the update
manager alone would have left both fixed and unreachable.

**Which made restore part of this task.** Exposing "Restore" without fixing
`untrash_object` — same server-id guard — would have been worse than leaving it
hidden. And it is load-bearing beyond the menu: T4.4f's safety argument is that
an object missing from the tree is *trashed rather than deleted, because
trashing is recoverable*, which was not true here. A trash you cannot restore
from is a delete with extra steps.

Restoring moves an object to the root when its folder is itself in the trash,
because restoring into a trashed folder restores it *into* the trash, where the
user cannot see it and has no way to find out where it went. Not an invention:
upstream's `test_metadata_after_untrash_item_and_move_to_root` asserts the
server answers exactly this way. With no server, the client decides it.

One predicate for all four verbs, `fork::drive_deletes_are_local`, because they
are one question — does removing an object need permission from somewhere else?
Answering it per-verb is how a trash you can fill but not empty comes about,
which is the state the fork was in between T4.4f and T4.7.

**Known limits, both narrow.** A local delete's completion event carries
`server_id: None`, because `ServerId::from_string_lossy` asserts 22 characters
and a client uid is 43 — it panics rather than lying, which the first version
of this found the hard way. So the two listeners keyed on `server_id` — the
environments page's success toast and `ambient_agents::scheduled`'s completion
channel — stay silent for a local object. Neither was reachable before, since
the delete they wait on never happened; `scheduled`'s waiter hangs either way,
which is its own upstream bug and not this one.

And T4.4f's empty-tree guard is now reachable in earnest: on a single-object
drive, deleting the object *and* emptying the trash produces an empty tree,
which the import refuses. The last deletion still cannot propagate.

#### Verified on Windows, 2026-08-19

Through the panel, on the running build, against the very object that could not
be got rid of — `from-another-machine`, trashed since the T4.4f session.

The first thing to check was the menu, since before this it had no entries at
all on a trashed object. Right-click now draws **Restore** and **Delete
forever**.

    Restore          -> trash empties, object back under PERSONAL,
                        "Empty trash" greys out
    drive export     -> written 1, unchanged 1
                        and the file no longer carries "trashed"

    Trash it again, then Delete forever
                     -> toast "1 object deleted forever"
    drive status     -> objects 2 -> 1
    drive export     -> removed_files 1

    Import a hand-authored throwaway (created 1), trash it,
    then the Empty trash button
                     -> confirmation dialog, then
                        "Trash emptied: 1 object deleted forever"
    drive export     -> removed_files 1

Then closed and reopened Warp: `objects 1`, export `unchanged 1`. Nothing came
back, which is the part only a restart can show — the in-memory model and the
panel would look identical either way, and the SQLite delete is what makes it
permanent.

The toasts are worth noting rather than skipping past: they are driven by the
completion event, so seeing them is what confirms the local path emits it. The
`drive export` numbers are the independent check — `removed_files 1` means the
object was gone from the *store*, not just from the panel.

Driven with a new `C:\dev\click.ps1` (see `.fork/docs/manual.md`), because the trash
menu has no `warpctrl` action and no keybinding: this is the first fork
behaviour that could only be reached through the GUI.

### The alias gap — T4.4g

The live run found something reading would not have. The exported workflow has
no alias, and `wf-test` was set on it.

Not a defect in the format: **workflow aliases are not drive objects at all.**
`WorkflowAliases` (`workflows/aliases.rs`) is a *settings group* — a
`Vec<WorkflowAlias>` under storage key `WorkflowAliases`, each entry holding an
`alias` string and the `workflow_id: SyncId` it points at. So the export is
lossless with respect to the drive; the alias was never in it.

It is still a real gap for what T4.4 is *for*. Carry the repository to another
machine, import, and the workflow returns without its alias — because aliases
live in settings, which in this fork sync nowhere.

Tractable, and the format already did the hard part: aliases reference
workflows by `SyncId`, and a `SyncId` is exactly what the files preserve. So
the link would survive if the aliases travelled. What needs deciding is whether
settings-shaped data belongs in a *drive* mirror at all, and what happens to an
alias pointing at a workflow that is not in the personal space. That is a
design call, not a line of code, which is why it is its own task.

### T4.4g as built — the alias travels in the workflow's file

**Where it goes was the whole question.** The obvious answer is a side-car:
mirror the settings group as a top-level `.warp-aliases.json`. Everything that
makes the rest of this format work argues against it. Placement is the path,
identity is in the file, deleting the file deletes the thing — a list of ids has
none of those properties. Delete a workflow's file and the side-car entry is
left pointing at nothing; and a list of ids is exactly the shape a diff cannot
review.

Carried in the workflow's own file, an alias moves when the workflow moves, dies
when it dies, and **cannot dangle, because there is nowhere for it to dangle
from**. The cost is that `PortableObject` is now a join of two sources rather
than a projection of one object's rows — done in `snapshot`, which is the layer
whose entire job is bridging the store to this form.

**The import rule is deliberately unlike the object rule**, and this is the part
worth reading twice. Objects read absence as deletion, which works *because* a
deleted object still exports as a trashed one — absence therefore means
something specific. An alias has no such tombstone: an alias that is gone is
just gone. So absence is only read **within the workflows the tree describes**.
An alias pointing anywhere else — a team workflow, an object outside the mirror
— is left completely alone, because reconciling the whole list against the tree
would wipe it with nothing anywhere to restore it from.

Three smaller decisions, each because the alternative breaks byte stability or
loses data:

- **`arguments` becomes a `BTreeMap`.** The setting holds a `HashMap`, and
  `serde_json` writes a map in iteration order — which for a `HashMap` is
  randomised per process. The same alias would have produced different bytes
  after every restart, and the repository would never have been clean twice
  running. The alias *list* is sorted for the same reason: reordering two
  aliases must not be a diff.
- **The format version goes to 2.** Not ceremony. A v1 build reading a v2 file
  ignores `aliases`, and its next export writes the file back without them — so
  a build that believed it was doing nothing would destroy them. Refusing the
  whole file is the only reading of a version number that protects against that.
  Old files still read; the bump only stops old *builds*.
- **`env_vars` is the one sideways reference the format keeps as an id.**
  Placement is a path because it points at a container; an env var collection is
  a sibling. It resolves after an import because the collection is itself a
  drive object travelling in the same tree.

Aliases whose workflow is not in the mirror are counted as `aliases_not_mirrored`
by `status` and `export`, so "why didn't my alias travel" has an answer. A tree
that claims an alias currently held by a workflow outside the mirror takes it —
two `dep`s is not a state — and that is reported by name under
`aliases_reassigned`, since it changes something the tree does not describe.

**Known edge, inherited rather than introduced:** `WorkflowAliases::connect`
drops a workflow's aliases when it is trashed, so an import that trashes a
workflow loses its aliases permanently — restoring it from the panel does not
bring them back. Identical to trashing a workflow from the GUI, so it is
upstream behaviour rather than a mirror bug, but it is worth knowing.

9 tests, 6 of which were confirmed to fail with the join stubbed out. The other
three are guard tests — "an alias outside the tree is left alone", the
idempotence check — which pass trivially when the feature does nothing, which is
what a guard test is for.

#### Verified on Windows, 2026-08-19

Against the very alias whose absence started this task — `wf-test`, on
`simple-workflow-test`:

    export     written 2      <- every file rewritten once, for the v2 bump
    the file now carries
        "aliases": [ { "alias": "wf-test", "arguments": {} } ]

    (rename it to "wft" in the file, as the other machine would have)
    import     aliases_removed 1, aliases_set 1, updated 1
    export     unchanged 2, written 0

    (rename it back)
    import     aliases_removed 1, aliases_set 1
    import     unchanged 2, updated 0, no alias counters at all

That `export → unchanged 2, written 0` immediately after the import is the
line that proves it. If the alias had not actually reached the settings store,
the export would have written the file straight back to `wf-test`.
### T4.4e as built — what happens when git leaves a conflict behind

Decision 1 settles *who* resolves a conflict: the user, in their own
repository, with the tools they already have. It says nothing about what Warp
does when it **meets** one, and that was a side effect rather than a decision.

It was also a bug, and a bad one. A file with `<<<<<<<` in it does not parse, so
it landed in `ignored` next to the user's README, so the object it describes was
absent from the tree — and absence is exactly how T4.4f's import is told an
object was deleted. **The objects in the middle of being merged were the ones
that got trashed.** Nothing about the old behaviour announced this; the import
reported success and a trash count.

The policy is three rules, and the first is the one the other two serve.

**Warp never resolves a conflict, and never guesses.** Both sides are
reconstructed, but only to answer "is this file one of mine?" — never to pick
one. Choosing a side is the merge behaviour decision 1 rejected, and it would
happen silently, on the one occasion the user is demonstrably already looking at
the file. `--ours` and `--theirs` are git's words and they belong to the user.

**Both directions refuse, whole.** Import stops rather than skipping the
conflicted files, for the reason above. Export stops rather than overwriting
them, because the half-merged file is the only copy of the merge in front of the
user and git will not put it back for them. All-or-nothing in both cases: the
export reads every file it would write *before* writing any of them, so a
refusal never leaves half a drive on disk. That pre-read is not extra work — the
"is this file already correct" check needed it anyway.

**Only our files count.** Ours-ness is decided by parsing each side, not by
spotting a marker. The mirror shares a repository with the user's own work, and
their conflicted README is not ours to have an opinion about — it stays in
`ignored`, and an export runs straight past it.

Two narrowings in the detector, both aimed at not crying conflict over a good
file. A region must be **closed**: an opening marker alone is somebody writing
*about* merges. And `=======` counts as a separator only between markers,
because a bare row of equals signs is a markdown setext `<h1>` — and notebooks
are markdown, so reading one as a conflict would make every notebook written in
that style unimportable. The diff3 `|||||||` ancestor is parsed and discarded;
it is neither side.

The refusal is a type (`tree::ConflictsInTheWay`) rather than a message, so
`drive.sync.export` can answer `invalid_request` — your tree is mid-merge, ten
seconds to fix — instead of `internal`, which would send the user to look at the
wrong thing entirely. Both directions produce the same sentence, naming
`path:line (object name)`. The name is the point: "resolve `deploy-a1b2c3d4.json`"
is a chore handed to someone who has to work out what it is first.

`drive.sync.status` now reads the tree as well as the store, and lists the
conflicted files. It already described itself as the action you run to find out
why an export will not run, and an unresolved merge is the only condition that
stops *both* directions — a status that could not see it would send the user to
inspect the setting.

13 tests, all confirmed to fail with the detector stubbed out.

#### Verified on Windows, 2026-08-19

Against the same real repository, by hand-writing a genuine merge conflict into
the exported `simple-workflow-test` file — both sides real, differing in the
command:

    status     conflicted: ["...\simple-workflow-test-8f89f76f.json:1
                            (simple-workflow-test)"]
    import     invalid_request: 1 file(s) under C:\dev\warp-drive-mirror have
               unresolved merge conflicts
    export     invalid_request: the same sentence

    (the file still has its markers — the export did not overwrite the merge)

    (resolve to our side)
    status     no conflicted key
    import     updated 1, unchanged 1, trashed 0   <- the workflow survived
    export     written 1, unchanged 1

`trashed: 0` is the line that matters. Before this, the refused import would
have been a successful import that trashed `simple-workflow-test`.

Then the other half, with a real conflict in the user's own `README.md`:

    status     no conflicted key
    export     unchanged 2, written 0
    import     unchanged 2, trashed 0, and README.md in ignored with
               "unresolved merge conflict at line 1, and neither side is a
                Warp Drive file"

Their merge, their file, their business — and it stops nothing.

Two things the tests caught that reading would not have: serde_yaml 0.8 opens
its output with a document-start marker, which is the same three characters as
the front-matter fence and becomes the *closing* fence on read; and
`user:local` contains a colon, so YAML quotes it.

