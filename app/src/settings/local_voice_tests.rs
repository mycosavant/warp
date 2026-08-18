use settings::{Setting, SyncToCloud};
use settings_value::SettingsValue;

use super::*;
use crate::voice::local_transcriber::build_command_args;

/// The point of the feature is that audio stays on the machine, so the
/// out-of-the-box endpoint must be loopback. A default pointing anywhere else
/// would ship voice off-device for anyone who never opens settings.
#[test]
fn the_default_endpoint_is_loopback() {
    let endpoint = LocalVoiceEndpoint::default_value();
    let url = reqwest::Url::parse(&endpoint).expect("default endpoint is a valid URL");
    assert_eq!(url.host_str(), Some("127.0.0.1"), "{endpoint}");
}

/// whisper.cpp's `whisper-server` serves `/inference`, not the OpenAI route —
/// verified against a live server, not assumed.
#[test]
fn the_default_endpoint_targets_whisper_cpps_route() {
    assert!(LocalVoiceEndpoint::default_value().ends_with("/inference"));
}

#[test]
fn the_default_backend_is_http() {
    assert_eq!(
        LocalVoiceBackendSetting::default_value(),
        LocalVoiceBackend::Http
    );
    assert_eq!(
        LocalVoiceBackend::Http.to_file_value(),
        serde_json::json!("http")
    );
    assert_eq!(
        LocalVoiceBackend::from_file_value(&serde_json::json!("command")).unwrap(),
        LocalVoiceBackend::Command
    );
}

/// The shipped argument template has to survive the parser it is fed to, or
/// the command backend is broken the first time anyone selects it.
#[test]
fn the_default_command_args_are_accepted_by_the_parser() {
    let args = build_command_args(
        &LocalVoiceCommandArgs::default_value(),
        "/models/ggml-base.bin",
        Some("en"),
    )
    .expect("the shipped template must parse");
    assert!(args.iter().any(|arg| arg == "{audio}"));
    assert!(args.iter().any(|arg| arg == "/models/ggml-base.bin"));
    assert!(args.iter().any(|arg| arg == "en"));
}

/// No transcriber runs until the user picks one, so `command` starts empty and
/// its error path — not a guess at a binary name — is what they hit.
#[test]
fn the_command_and_model_start_unset() {
    assert!(LocalVoiceCommand::default_value().is_empty());
    assert!(LocalVoiceModel::default_value().is_empty());
}

/// These name a path or port on one machine. Syncing them would push local
/// filesystem layout to a server the fork otherwise never talks to.
#[test]
fn nothing_here_syncs_to_the_cloud() {
    assert_eq!(
        LocalVoiceBackendSetting::sync_to_cloud(),
        SyncToCloud::Never
    );
    assert_eq!(LocalVoiceEndpoint::sync_to_cloud(), SyncToCloud::Never);
    assert_eq!(LocalVoiceModel::sync_to_cloud(), SyncToCloud::Never);
    assert_eq!(LocalVoiceCommand::sync_to_cloud(), SyncToCloud::Never);
    assert_eq!(LocalVoiceCommandArgs::sync_to_cloud(), SyncToCloud::Never);
}

#[test]
fn every_setting_lives_under_the_same_toml_table() {
    for path in [
        LocalVoiceBackendSetting::toml_path(),
        LocalVoiceEndpoint::toml_path(),
        LocalVoiceModel::toml_path(),
        LocalVoiceCommand::toml_path(),
        LocalVoiceCommandArgs::toml_path(),
    ] {
        let path = path.expect("local voice settings are user-visible");
        assert!(
            path.starts_with("agents.voice.local_transcription."),
            "{path}"
        );
    }
}

#[test]
fn accessors_trim_surrounding_whitespace() {
    let settings = LocalVoiceSettings {
        local_voice_backend: LocalVoiceBackendSetting::new(Some(LocalVoiceBackend::Command)),
        local_voice_endpoint: LocalVoiceEndpoint::new(Some("  http://127.0.0.1:9000/x  ".into())),
        local_voice_model: LocalVoiceModel::new(Some("  base  ".into())),
        local_voice_command: LocalVoiceCommand::new(Some("  /usr/bin/whisper-cli\n".into())),
        local_voice_command_args: LocalVoiceCommandArgs::new(Some(" -f {audio} ".into())),
    };
    assert_eq!(settings.backend(), LocalVoiceBackend::Command);
    assert_eq!(settings.endpoint(), "http://127.0.0.1:9000/x");
    assert_eq!(settings.model(), "base");
    assert_eq!(settings.command(), "/usr/bin/whisper-cli");
    assert_eq!(settings.command_args(), "-f {audio}");
}
