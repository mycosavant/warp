use warp_util::standardized_path::StandardizedPath;
use warpui_core::App;

use super::*;
use crate::StandingQueryContent;

fn path(path: &str) -> StandardizedPath {
    StandardizedPath::try_new(path).unwrap()
}

#[test]
fn snapshot_and_incremental_update_maintain_remote_standing_results() {
    App::test((), |mut app| async move {
        let model = app.add_model(RemoteRepoMetadataModel::new);
        let host = HostId::new("remote-host".to_string());
        let repo_path = path("/repo");
        let skill = StandingQueryContent::file(path("/repo/.agents/skills/review/SKILL.md"));
        let rule = StandingQueryContent::file(path("/repo/WARP.md"));
        let id = RemoteRepositoryIdentifier::new(host.clone(), repo_path.clone());
        let snapshot = RepoMetadataUpdate {
            repo_path: repo_path.clone(),
            remove_entries: Vec::new(),
            update_entries: Vec::new(),
            standing_results_delta: StandingQueryResultsDelta {
                upserted_project_skills: vec![skill.clone()],
                ..Default::default()
            },
        };
        model.update(&mut app, |model, ctx| {
            model.insert_from_snapshot(host.clone(), &snapshot, ctx);
        });

        let incremental = RepoMetadataUpdate {
            repo_path,
            remove_entries: Vec::new(),
            update_entries: Vec::new(),
            standing_results_delta: StandingQueryResultsDelta {
                removed_project_skills: vec![skill],
                upserted_project_rules: vec![rule.clone()],
                ..Default::default()
            },
        };
        model.update(&mut app, |model, ctx| {
            model.apply_incremental_update(&host, &incremental, ctx);
        });

        model.read(&app, |model, _ctx| {
            let results = model.standing_query_results(&id).unwrap();
            assert!(results.project_skills().next().is_none());
            assert!(results.project_rules().any(|content| content == &rule));
        });
    });
}

#[test]
fn an_update_before_the_snapshot_is_dropped_and_the_snapshot_still_wins() {
    // The ordering the server produces on every navigation, and which the
    // client cannot avoid: the repository *root* is the key an update is filed
    // under, and only the server knows it until the snapshot arrives. The
    // watcher fires during indexing, so early updates name a key the client
    // has never seen.
    //
    // What must hold is that dropping them costs nothing: the snapshot is a
    // complete state rather than a delta, so the state afterwards is the same
    // as if the early update had never been sent. This is the assumption the
    // downgraded log line rests on, so it is pinned rather than asserted in a
    // comment.
    App::test((), |mut app| async move {
        let model = app.add_model(RemoteRepoMetadataModel::new);
        let host = HostId::new("remote-host".to_string());
        let repo_path = path("/repo");
        let id = RemoteRepositoryIdentifier::new(host.clone(), repo_path.clone());
        let rule = StandingQueryContent::file(path("/repo/WARP.md"));

        let early = RepoMetadataUpdate {
            repo_path: repo_path.clone(),
            remove_entries: Vec::new(),
            update_entries: Vec::new(),
            standing_results_delta: StandingQueryResultsDelta {
                upserted_project_rules: vec![rule.clone()],
                ..Default::default()
            },
        };
        model.update(&mut app, |model, ctx| {
            model.apply_incremental_update(&host, &early, ctx);
        });
        // Nothing was invented for a key the server never announced.
        model.read(&app, |model, _| {
            assert!(
                model.repository_state(&id).is_none(),
                "an update for an unannounced repository must not register it",
            );
        });

        let snapshot = RepoMetadataUpdate {
            repo_path: repo_path.clone(),
            remove_entries: Vec::new(),
            update_entries: Vec::new(),
            standing_results_delta: StandingQueryResultsDelta {
                upserted_project_rules: vec![rule.clone()],
                ..Default::default()
            },
        };
        model.update(&mut app, |model, ctx| {
            model.insert_from_snapshot(host.clone(), &snapshot, ctx);
        });

        model.read(&app, |model, _| {
            let results = model
                .standing_query_results(&id)
                .expect("snapshot registers the repository");
            let rules: Vec<_> = results.project_rules().collect();
            assert_eq!(
                rules.len(),
                1,
                "the snapshot carries the state the dropped update described",
            );
        });
    });
}
