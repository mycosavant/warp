use serde_json::json;

use super::*;

/// A fixed client id, so every golden assertion below is reproducible.
const TEST_CLIENT_UID: &str = "Client-4f2a1c8e-0d3b-4a76-9c11-8e5b2d7f6a90";
/// [`ServerId`] is exactly 22 characters; anything else is a parse error.
const TEST_SERVER_UID: &str = "aBcDeFgHiJkLmNoPqRsTuV";
const TEST_TEAM_UID: &str = "zYxWvUtSrQpOnMlKjIhGfE";

/// The full byte content of a workflow file, pinned.
///
/// This is the test that fails when something starts leaking into the format.
/// The store has plenty of fields that must never appear here — `is_pending`,
/// `retry_count`, `current_editor`, the local integer row ids — and none of
/// them are visible from inside a round-trip test, because a round trip is
/// equally happy carrying a field that should not exist.
#[test]
fn a_workflow_file_looks_like_this() {
    let object = workflow_fixture();

    assert_eq!(
        object.to_file_contents().unwrap(),
        r#"{
  "warp_drive": 2,
  "type": "WORKFLOW",
  "uid": "Client-4f2a1c8e-0d3b-4a76-9c11-8e5b2d7f6a90",
  "name": "simple-workflow-test",
  "owner": "user:local",
  "revision": "2025-08-18T19:14:16.123456Z",
  "data": {
    "arguments": [],
    "command": "echo hello world",
    "name": "simple-workflow-test"
  }
}
"#
    );
}

/// The other half: a notebook is prose, and prose in a JSON string is prose no
/// diff can read. Front matter keeps the markdown as markdown.
#[test]
fn a_notebook_file_keeps_its_markdown_as_markdown() {
    let object = notebook_fixture("# Title\n\nSome *prose*.\n");

    assert_eq!(
        object.to_file_contents().unwrap(),
        r#"---
warp_drive: 2
type: NOTEBOOK
uid: Client-4f2a1c8e-0d3b-4a76-9c11-8e5b2d7f6a90
name: Field notes
owner: "user:local"
---
# Title

Some *prose*.
"#
    );
}

/// The core guarantee. Every object type, through the file and back, unchanged.
#[test]
fn every_object_type_round_trips() {
    for object in one_of_every_object_type() {
        let contents = object.to_file_contents().unwrap();
        let parsed = PortableObject::from_file_contents(&contents)
            .unwrap_or_else(|err| panic!("{:?} failed to parse: {err:#}", object.object_type));

        assert_eq!(
            parsed, object,
            "{:?} did not survive a round trip",
            object.object_type
        );
    }
}

/// The property the whole format exists to serve: an object that has not
/// changed must produce the bytes it produced last time, or `git status` is
/// permanently dirty and the store is unusable as a synced repository.
#[test]
fn an_unchanged_object_produces_unchanged_bytes() {
    for object in one_of_every_object_type() {
        let first = object.to_file_contents().unwrap();
        let second = PortableObject::from_file_contents(&first)
            .unwrap()
            .to_file_contents()
            .unwrap();

        assert_eq!(
            first, second,
            "{:?} churns across a reload",
            object.object_type
        );
    }
}

/// `serde_json`'s maps are `BTreeMap`, so key order is sorted rather than
/// insertion order. Pinned because the workspace enabling `preserve_order`
/// would silently make every file's byte-stability depend on hash iteration.
#[test]
fn json_payload_keys_are_sorted_not_insertion_ordered() {
    let mut object = workflow_fixture();
    object.payload = Payload::Json(json!({ "zebra": 1, "apple": 2, "mango": 3 }));

    let contents = object.to_file_contents().unwrap();
    let keys: Vec<_> = ["apple", "mango", "zebra"]
        .iter()
        .map(|key| contents.find(key).expect("key is present"))
        .collect();

    assert!(keys.windows(2).all(|pair| pair[0] < pair[1]), "{contents}");
}

/// Markdown is full of `---`: horizontal rules, and front matter someone pasted
/// in from somewhere else. The closing fence is the first one after the header,
/// never the last, so none of that reaches the parser.
#[test]
fn a_notebook_body_may_contain_front_matter_fences() {
    let markdown = "intro\n\n---\n\n---\nnot: a header\n---\n\noutro\n";
    let object = notebook_fixture(markdown);

    let parsed = PortableObject::from_file_contents(&object.to_file_contents().unwrap()).unwrap();

    assert_eq!(parsed, object);
    assert_eq!(
        parsed.payload,
        Payload::Notebook {
            markdown: markdown.to_owned(),
            ai_document_id: None
        }
    );
}

/// The nastier direction: a fence inside the *header*. A notebook titled with a
/// line that is exactly `---` would truncate its own front matter, so the
/// header falls back to JSON — which is valid YAML and escapes newlines, so it
/// cannot contain a bare fence at all.
#[test]
fn a_title_containing_a_fence_does_not_truncate_the_header() {
    let mut object = notebook_fixture("body\n");
    object.name = "before\n---\nafter".to_owned();

    let contents = object.to_file_contents().unwrap();
    let parsed = PortableObject::from_file_contents(&contents).unwrap();

    assert_eq!(
        parsed, object,
        "the header was cut short at the embedded fence"
    );
    assert_eq!(
        parsed.payload,
        Payload::Notebook {
            markdown: "body\n".to_owned(),
            ai_document_id: None
        }
    );
}

/// Microseconds are what SQLite stores, so anything coarser silently rewrites
/// every timestamp on the first import and dirties the whole tree at once.
#[test]
fn timestamps_keep_microsecond_precision() {
    let mut object = workflow_fixture();
    object.revision_ts = Some(1_755_544_456_000_001);
    object.metadata_last_updated_ts = Some(1_755_544_456_999_999);
    object.trashed_ts = Some(-1);

    let parsed = PortableObject::from_file_contents(&object.to_file_contents().unwrap()).unwrap();

    assert_eq!(parsed.revision_ts, object.revision_ts);
    assert_eq!(
        parsed.metadata_last_updated_ts,
        object.metadata_last_updated_ts
    );
    assert_eq!(parsed.trashed_ts, object.trashed_ts);
}

/// Two objects sharing a name is ordinary — a "deploy" workflow in two folders,
/// or two rules both called "style". The filename has to stay a pure function
/// of the object: disambiguating against siblings would mean creating the
/// second one renames the first.
#[test]
fn objects_with_the_same_name_get_different_file_names() {
    let first = workflow_fixture();
    let mut second = workflow_fixture();
    second.id = SyncId::ServerId(ServerId::try_from(TEST_SERVER_UID).unwrap());

    assert_ne!(first.file_name(), second.file_name());
    assert!(first.file_name().starts_with("simple-workflow-test-"));
    assert!(second.file_name().starts_with("simple-workflow-test-"));
}

/// The tree is browsed by a human and checked out on Windows, so a name is a
/// slug and never a path.
#[test]
fn names_become_filesystem_safe_slugs() {
    let cases = [
        ("Deploy to prod", "deploy-to-prod"),
        ("../../etc/passwd", "etc-passwd"),
        ("  leading and trailing  ", "leading-and-trailing"),
        ("Ünïcödé ✨", "n-c-d"),
        ("!!!", UNNAMED_SLUG),
        ("", UNNAMED_SLUG),
    ];

    for (name, expected) in cases {
        assert_eq!(slug(name), expected, "slug of {name:?}");
    }

    let long = slug(&"a".repeat(500));
    assert!(long.len() <= MAX_SLUG_LEN, "slug was {} chars", long.len());
}

/// A folder is a directory, and its metadata lives inside it. That is what
/// makes a move show up as a rename rather than as a changed field, and what
/// makes the tree browsable without Warp.
#[test]
fn a_folder_owns_a_directory_and_a_metadata_file() {
    let folder = folder_fixture();

    assert_eq!(folder.file_name(), FOLDER_FILE_NAME);
    assert_eq!(
        folder.folder_directory_name().as_deref(),
        Some("scripts-".to_owned() + &uid_hash(&folder.id)).as_deref()
    );
    assert_eq!(workflow_fixture().folder_directory_name(), None);
}

/// The sqlite type string is the format's type string, so every variant has to
/// survive the trip. The `match` below has no wildcard: a new upstream variant
/// fails to compile here rather than quietly going untested.
#[test]
fn every_object_type_string_round_trips() {
    for object_type in all_object_types() {
        let as_str = object_type_to_str(object_type);
        assert_eq!(
            object_type_from_str(&as_str).unwrap(),
            object_type,
            "{as_str} did not parse back"
        );
    }
}

#[test]
fn both_kinds_of_owner_round_trip() {
    let user = Owner::User {
        user_uid: UserUid::new("local"),
    };
    let team = Owner::Team {
        team_uid: ServerId::try_from(TEST_TEAM_UID).unwrap(),
    };

    assert_eq!(owner_from_str(&owner_to_str(&user)).unwrap(), user);
    assert_eq!(owner_from_str(&owner_to_str(&team)).unwrap(), team);
}

/// `ServerId::from_string_lossy` panics in debug builds on anything that is not
/// exactly 22 characters, and these files come off a disk the user edits.
/// A hand-mangled id has to be an error the caller can report.
#[test]
fn a_malformed_file_is_an_error_rather_than_a_panic() {
    let cases = [
        r#"{"warp_drive":2,"type":"WORKFLOW","uid":"Client-4f2a1c8e-0d3b-4a76-9c11-8e5b2d7f6a90","name":"x","owner":"team:too-short","data":{}}"#,
        r#"{"warp_drive":2,"type":"WORKFLOW","uid":"not-an-id","name":"x","owner":"user:local","data":{}}"#,
        r#"{"warp_drive":2,"type":"NOT_A_TYPE","uid":"Client-4f2a1c8e-0d3b-4a76-9c11-8e5b2d7f6a90","name":"x","owner":"user:local","data":{}}"#,
        r#"{"warp_drive":2,"type":"WORKFLOW","uid":"Client-4f2a1c8e-0d3b-4a76-9c11-8e5b2d7f6a90","name":"x","owner":"nobody","data":{}}"#,
        // A workflow with no payload at all.
        r#"{"warp_drive":2,"type":"WORKFLOW","uid":"Client-4f2a1c8e-0d3b-4a76-9c11-8e5b2d7f6a90","name":"x","owner":"user:local"}"#,
        "not a file we wrote",
    ];

    for contents in cases {
        assert!(
            PortableObject::from_file_contents(contents).is_err(),
            "expected a parse error for {contents}"
        );
    }
}

/// Refusing a newer file is the difference between the format being versioned
/// and the version number being decoration: a v2 file half-understood by a v1
/// build would be written back with its unknown fields dropped.
#[test]
fn a_file_from_a_newer_format_is_refused() {
    let contents = workflow_fixture()
        .to_file_contents()
        .unwrap()
        .replace("\"warp_drive\": 2", "\"warp_drive\": 3");

    let err = PortableObject::from_file_contents(&contents).unwrap_err();

    assert!(format!("{err:#}").contains("newer"), "{err:#}");
}

/// T4.4g, pinned. An alias is a shortcut the user typed, and it belongs in the
/// file of the thing it is a shortcut *to* — so it moves, and dies, with it.
///
/// The order is the assertion that matters: aliases come out sorted, not in the
/// order they were added, or reordering two of them would be a diff.
#[test]
fn a_workflows_aliases_are_carried_in_its_own_file() {
    let mut object = workflow_fixture();
    object.aliases = vec![
        Alias {
            alias: "dep".to_owned(),
            env_vars: Some(TEST_SERVER_UID.to_owned()),
            arguments: Some(BTreeMap::from([("target".to_owned(), "prod".to_owned())])),
        },
        Alias {
            alias: "b".to_owned(),
            env_vars: None,
            arguments: None,
        },
    ];

    assert_eq!(
        object.to_file_contents().unwrap(),
        r#"{
  "warp_drive": 2,
  "type": "WORKFLOW",
  "uid": "Client-4f2a1c8e-0d3b-4a76-9c11-8e5b2d7f6a90",
  "name": "simple-workflow-test",
  "owner": "user:local",
  "revision": "2025-08-18T19:14:16.123456Z",
  "aliases": [
    {
      "alias": "b"
    },
    {
      "alias": "dep",
      "env_vars": "aBcDeFgHiJkLmNoPqRsTuV",
      "arguments": {
        "target": "prod"
      }
    }
  ],
  "data": {
    "arguments": [],
    "command": "echo hello world",
    "name": "simple-workflow-test"
  }
}
"#
    );
    assert_eq!(
        PortableObject::from_file_contents(&object.to_file_contents().unwrap())
            .unwrap()
            .aliases
            .len(),
        2
    );
}

/// Nothing but a workflow can be aliased, so an `aliases` key written onto
/// anything else by hand is dropped rather than carried into settings as an
/// entry pointing at something that can never answer to it.
#[test]
fn an_alias_on_something_that_is_not_a_workflow_is_dropped() {
    let mut notebook = notebook_fixture("body\n");
    notebook.aliases = vec![Alias {
        alias: "notes".to_owned(),
        env_vars: None,
        arguments: None,
    }];

    let parsed = PortableObject::from_file_contents(&notebook.to_file_contents().unwrap()).unwrap();

    assert!(parsed.aliases.is_empty());
}

/// A mirror written before T4.4g. Reading it must work — the version bump is
/// there to stop an *old* build mangling a new file, not to strand old ones.
#[test]
fn a_file_from_before_aliases_still_reads() {
    let contents = workflow_fixture()
        .to_file_contents()
        .unwrap()
        .replace("\"warp_drive\": 2", "\"warp_drive\": 1");

    let parsed = PortableObject::from_file_contents(&contents).unwrap();

    assert!(parsed.aliases.is_empty());
    assert_eq!(parsed.name, "simple-workflow-test");
}

/// A real `git merge` result, split back into the two files that produced it.
///
/// The point of reconstructing both sides is not to pick one. It is that a
/// caller can ask "is this one of mine?" of a file that no parser will touch,
/// and get a truthful answer about a workflow rather than a shrug.
#[test]
fn a_conflicted_file_yields_both_sides() {
    let ours = workflow_fixture().to_file_contents().unwrap();
    let theirs = ours.replace("echo hello world", "echo goodbye");
    let merged = merge_markers(&ours, &theirs, None);

    let conflict = conflict(&merged).expect("this is a merge conflict");

    assert_eq!(conflict.ours, ours);
    assert_eq!(conflict.theirs, theirs);
    assert_eq!(
        PortableObject::from_file_contents(&conflict.ours).unwrap(),
        workflow_fixture()
    );
}

/// `merge.conflictStyle = diff3` adds a third section. The common ancestor is
/// neither side, so it must not leak into either reconstruction.
#[test]
fn the_diff3_common_ancestor_is_not_mistaken_for_a_side() {
    let ours = workflow_fixture().to_file_contents().unwrap();
    let theirs = ours.replace("echo hello world", "echo goodbye");
    let base = ours.replace("echo hello world", "echo original");
    let merged = merge_markers(&ours, &theirs, Some(&base));

    let conflict = conflict(&merged).expect("diff3 output is still a conflict");

    assert_eq!(conflict.ours, ours);
    assert_eq!(conflict.theirs, theirs);
    assert!(!conflict.ours.contains("echo original"));
    assert!(!conflict.theirs.contains("echo original"));
}

/// The false positive that would matter most, because notebooks are markdown
/// and this is how markdown underlines a heading.
///
/// A row of equals signs is a setext `<h1>`. Reading one as a conflict
/// separator would make every notebook written in that style unimportable.
#[test]
fn a_setext_heading_is_not_a_conflict() {
    let notebook = notebook_fixture("Release notes\n=============\n\nShipped it.\n");

    let contents = notebook.to_file_contents().unwrap();

    assert_eq!(conflict(&contents), None);
    assert_eq!(
        PortableObject::from_file_contents(&contents).unwrap(),
        notebook
    );
}

/// Prose about merge conflicts is prose, not a merge conflict. An unclosed
/// marker is somebody writing about the thing rather than being in it.
#[test]
fn an_unclosed_marker_is_not_a_conflict() {
    let cases = [
        "Git writes <<<<<<< HEAD when it cannot merge.\n",
        "<<<<<<< HEAD\nhalf a region and no end to it\n=======\n",
        ">>>>>>> theirs\nan end with no beginning\n",
        "=======\n",
        "<<<<<< six is not enough\n=======\n>>>>>> six\n",
    ];

    for contents in cases {
        assert_eq!(conflict(contents), None, "{contents:?}");
    }
}

/// The line number is the whole value of the report: it is what turns "one of
/// your files is broken" into somewhere to put the cursor.
#[test]
fn the_reported_line_is_where_the_conflict_opens() {
    let contents = "one\ntwo\n<<<<<<< HEAD\nmine\n=======\ntheirs\n>>>>>>> branch\n";

    let conflict = conflict(contents).unwrap();

    assert_eq!(conflict.line, 3);
    assert_eq!(conflict.ours, "one\ntwo\nmine\n");
    assert_eq!(conflict.theirs, "one\ntwo\ntheirs\n");
}

/// Git labels the outer markers and sizes them by `conflict-marker-size`, so
/// the detector matches a run and a label rather than a literal seven.
#[test]
fn longer_markers_and_labels_are_still_markers() {
    let contents = "<<<<<<<<<<< ours\nmine\n===========\ntheirs\n>>>>>>>>>>> theirs\n";

    let conflict = conflict(contents).expect("a widened marker is still a marker");

    assert_eq!(conflict.ours, "mine\n");
    assert_eq!(conflict.theirs, "theirs\n");
}

/// Renders what git leaves in the working tree when two edits collide.
fn merge_markers(ours: &str, theirs: &str, base: Option<&str>) -> String {
    let base = base
        .map(|base| format!("||||||| merged common ancestors\n{base}"))
        .unwrap_or_default();
    format!("<<<<<<< HEAD\n{ours}{base}=======\n{theirs}>>>>>>> theirs\n")
}

fn workflow_fixture() -> PortableObject {
    PortableObject {
        id: SyncId::ClientId(ClientId::from_hash(TEST_CLIENT_UID).unwrap()),
        object_type: ObjectType::Workflow,
        name: "simple-workflow-test".to_owned(),
        owner: Owner::User {
            user_uid: UserUid::new("local"),
        },
        revision_ts: Some(1_755_544_456_123_456),
        metadata_last_updated_ts: None,
        trashed_ts: None,
        creator_uid: None,
        last_editor_uid: None,
        is_welcome_object: false,
        aliases: Vec::new(),
        payload: Payload::Json(json!({
            "name": "simple-workflow-test",
            "command": "echo hello world",
            "arguments": [],
        })),
    }
}

fn notebook_fixture(markdown: &str) -> PortableObject {
    PortableObject {
        object_type: ObjectType::Notebook,
        name: "Field notes".to_owned(),
        revision_ts: None,
        payload: Payload::Notebook {
            markdown: markdown.to_owned(),
            ai_document_id: None,
        },
        ..workflow_fixture()
    }
}

fn folder_fixture() -> PortableObject {
    PortableObject {
        object_type: ObjectType::Folder,
        name: "Scripts".to_owned(),
        revision_ts: None,
        payload: Payload::Folder {
            is_warp_pack: false,
        },
        ..workflow_fixture()
    }
}

/// One populated object per type, with every optional field set on at least
/// one of them.
fn one_of_every_object_type() -> Vec<PortableObject> {
    let mut objects = vec![
        workflow_fixture(),
        notebook_fixture("# Notes\n\nbody\n"),
        folder_fixture(),
    ];

    // A fully-populated object, so no optional field goes untested.
    objects.push(PortableObject {
        metadata_last_updated_ts: Some(1_755_544_999_000_000),
        trashed_ts: Some(1_755_545_000_000_000),
        creator_uid: Some("local".to_owned()),
        last_editor_uid: Some("local".to_owned()),
        is_welcome_object: true,
        payload: Payload::Notebook {
            markdown: String::new(),
            ai_document_id: Some("doc-123".to_owned()),
        },
        ..notebook_fixture("")
    });

    objects.push(PortableObject {
        id: SyncId::ServerId(ServerId::try_from(TEST_SERVER_UID).unwrap()),
        owner: Owner::Team {
            team_uid: ServerId::try_from(TEST_TEAM_UID).unwrap(),
        },
        payload: Payload::Folder { is_warp_pack: true },
        ..folder_fixture()
    });

    objects.extend(all_json_object_types().into_iter().map(|json_type| {
        PortableObject {
            object_type: ObjectType::GenericStringObject(GenericStringObjectFormat::Json(
                json_type,
            )),
            name: json_type.as_str().to_owned(),
            payload: Payload::Json(json!({ "kind": json_type.as_str(), "nested": { "b": 2, "a": [1, null, true] } })),
            ..workflow_fixture()
        }
    }));

    objects
}

fn all_object_types() -> Vec<ObjectType> {
    let mut types = vec![
        ObjectType::Notebook,
        ObjectType::Workflow,
        ObjectType::Folder,
    ];
    types.extend(all_json_object_types().into_iter().map(|json_type| {
        ObjectType::GenericStringObject(GenericStringObjectFormat::Json(json_type))
    }));
    types
}

fn all_json_object_types() -> Vec<JsonObjectType> {
    let all = vec![
        JsonObjectType::Preference,
        JsonObjectType::EnvVarCollection,
        JsonObjectType::WorkflowEnum,
        JsonObjectType::AIFact,
        JsonObjectType::MCPServer,
        JsonObjectType::AIExecutionProfile,
        JsonObjectType::TemplatableMCPServer,
        JsonObjectType::CloudEnvironment,
        JsonObjectType::ScheduledAmbientAgent,
        JsonObjectType::CloudAgentConfig,
    ];

    // Not a formality: this `match` has no wildcard, so adding a variant
    // upstream breaks the build here instead of leaving a type with no file
    // representation and no failing test.
    for json_type in &all {
        match json_type {
            JsonObjectType::Preference
            | JsonObjectType::EnvVarCollection
            | JsonObjectType::WorkflowEnum
            | JsonObjectType::AIFact
            | JsonObjectType::MCPServer
            | JsonObjectType::AIExecutionProfile
            | JsonObjectType::TemplatableMCPServer
            | JsonObjectType::CloudEnvironment
            | JsonObjectType::ScheduledAmbientAgent
            | JsonObjectType::CloudAgentConfig => {}
        }
    }

    assert_eq!(all.len(), 10, "a json object type was added or removed");
    all
}

/// **The two halves agree about who may carry an alias.**
///
/// `from_parts` has always dropped aliases for anything that is not a
/// `Workflow`; `header()` used to write them for anything at all. A notebook
/// carrying one would therefore survive `to_file_contents` and vanish on the way
/// back in, making the module's own "these are inverses" claim false for it.
///
/// Not a live bug when found — `snapshot` keys its alias map on workflow ids, so
/// nothing else ever gets one. Pinned anyway, because that is an invariant this
/// module depends on and does not own, and the `preserve_order` regression found
/// the same day is what a silently-broken distant invariant costs.
#[test]
fn only_a_workflow_carries_its_aliases_into_the_file() {
    let mut object = workflow_fixture();
    object.object_type = ObjectType::Notebook;
    object.payload = Payload::Notebook {
        markdown: "# notes".to_owned(),
        ai_document_id: None,
    };
    object.aliases = vec![Alias {
        alias: "dep".to_owned(),
        env_vars: None,
        arguments: None,
    }];

    let contents = object.to_file_contents().expect("a notebook is writable");
    assert!(
        !contents.contains("dep"),
        "a non-workflow wrote an alias the reader would drop:\n{contents}"
    );

    let round_tripped = PortableObject::from_file_contents(&contents).expect("it parses back");
    assert!(round_tripped.aliases.is_empty());
}

/// **"At every depth" is a claim, and until this test it was only a comment.**
///
/// `json_payload_keys_are_sorted_not_insertion_ordered` proves sorting at depth
/// 1 and nothing more: its payload is flat, so a `sorted_keys` that sorted the
/// top level and returned everything else unchanged would pass it. So would
/// every other test in this file — `an_unchanged_object_produces_unchanged_bytes`
/// compares a file against its own re-parse, which is idempotency and not
/// sortedness, and passes with no sorting applied at any depth.
///
/// Found by an agent in Warp's own panel, asked to check that same-day fix
/// adversarially and specifically whether the test could pass on a wrong
/// implementation. It named the exact shape that would slip through. This is the
/// third guard in one day found narrower than the rule it guards.
///
/// Objects inside arrays are included because that is the case a `match` on
/// `Value::Object` alone silently drops.
#[test]
fn keys_are_sorted_inside_nested_objects_and_inside_arrays() {
    let mut object = workflow_fixture();
    object.payload = Payload::Json(json!({
        "zebra": {
            "yak": 1,
            "ant": { "wolf": 1, "bee": 2 },
        },
        "apple": [{ "quail": 1, "crow": 2 }],
    }));

    let contents = object.to_file_contents().expect("it writes");

    for (outer, inner) in [
        ("apple", "zebra"),
        ("ant", "yak"),
        ("bee", "wolf"),
        ("crow", "quail"),
    ] {
        let first = contents.find(outer).expect("the earlier key is present");
        let second = contents.find(inner).expect("the later key is present");
        assert!(
            first < second,
            "`{outer}` must precede `{inner}` — sorting stopped before this depth:\n{contents}"
        );
    }
}
