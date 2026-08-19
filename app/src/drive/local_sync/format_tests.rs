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
  "warp_drive": 1,
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
warp_drive: 1
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
        r#"{"warp_drive":1,"type":"WORKFLOW","uid":"Client-4f2a1c8e-0d3b-4a76-9c11-8e5b2d7f6a90","name":"x","owner":"team:too-short","data":{}}"#,
        r#"{"warp_drive":1,"type":"WORKFLOW","uid":"not-an-id","name":"x","owner":"user:local","data":{}}"#,
        r#"{"warp_drive":1,"type":"NOT_A_TYPE","uid":"Client-4f2a1c8e-0d3b-4a76-9c11-8e5b2d7f6a90","name":"x","owner":"user:local","data":{}}"#,
        r#"{"warp_drive":1,"type":"WORKFLOW","uid":"Client-4f2a1c8e-0d3b-4a76-9c11-8e5b2d7f6a90","name":"x","owner":"nobody","data":{}}"#,
        // A workflow with no payload at all.
        r#"{"warp_drive":1,"type":"WORKFLOW","uid":"Client-4f2a1c8e-0d3b-4a76-9c11-8e5b2d7f6a90","name":"x","owner":"user:local"}"#,
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
        .replace("\"warp_drive\": 1", "\"warp_drive\": 2");

    let err = PortableObject::from_file_contents(&contents).unwrap_err();

    assert!(format!("{err:#}").contains("newer"), "{err:#}");
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
