//! Fork-local settings for on-device voice transcription.
//!
//! Upstream has exactly one transcription path: `ServerVoiceTranscriber` POSTs
//! base64 WAV to `api.warp.dev`, which forwards it to Wispr Flow or OpenAI.
//! The `Provider` enum picks *Warp's upstream vendor*, not where inference
//! runs — so voice audio leaves the machine whichever provider is selected.
//!
//! These settings point transcription at something the user controls instead.
//! They are deliberately a separate group rather than fields on `AISettings`:
//! a new file cannot conflict on an upstream merge, and voice configuration is
//! not AI-model configuration.
//!
//! See `voice::local_transcriber` for the consumer and
//! `fork::local_voice_transcription_enabled` for the enablement policy.

use serde::{Deserialize, Serialize};
use settings::macros::define_settings_group;
use settings::{SupportedPlatforms, SyncToCloud};

/// whisper.cpp's `whisper-server` listens here with no arguments, and its
/// inference route is `/inference` rather than the OpenAI one. Servers that
/// speak the OpenAI shape (speaches, faster-whisper-server, LocalAI) expose
/// `/v1/audio/transcriptions` instead, which is why the whole URL is
/// configurable rather than just a host and port.
const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:8080/inference";

/// Matches whisper.cpp's `whisper-cli`. Tokens are split on whitespace *before*
/// placeholders are substituted, so a temporary path containing spaces stays a
/// single argument.
const DEFAULT_COMMAND_ARGS: &str =
    "--model {model} --language {language} --no-timestamps --file {audio}";

/// Where transcription runs.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Deserialize,
    Eq,
    PartialEq,
    schemars::JsonSchema,
    Serialize,
    settings_value::SettingsValue,
)]
#[serde(rename_all = "snake_case")]
#[schemars(
    description = "Which local transcription backend to use.",
    rename_all = "snake_case"
)]
pub enum LocalVoiceBackend {
    /// POST the recording to a local HTTP server as `multipart/form-data`.
    #[default]
    Http,
    /// Write the recording to a temporary file and run a transcriber binary
    /// over it, reading the transcript from stdout.
    Command,
}

impl LocalVoiceBackend {
    pub const ALL: [Self; 2] = [Self::Http, Self::Command];

    pub fn as_dropdown_label(self) -> &'static str {
        match self {
            Self::Http => "Local HTTP server",
            Self::Command => "Local command",
        }
    }
}

define_settings_group!(LocalVoiceSettings, settings: [
    local_voice_backend: LocalVoiceBackendSetting {
        type: LocalVoiceBackend,
        default: LocalVoiceBackend::default(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::ALL,
        private: false,
        toml_path: "agents.voice.local_transcription.backend",
        description: "Where voice input is transcribed: `http` posts to a local server, `command` runs a local binary.",
    },
    local_voice_endpoint: LocalVoiceEndpoint {
        type: String,
        default: DEFAULT_ENDPOINT.to_string(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::ALL,
        private: false,
        toml_path: "agents.voice.local_transcription.endpoint",
        description: "Full URL of the local transcription endpoint. whisper.cpp's whisper-server uses /inference; OpenAI-compatible servers use /v1/audio/transcriptions.",
    },
    local_voice_model: LocalVoiceModel {
        type: String,
        default: String::new(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::ALL,
        private: false,
        toml_path: "agents.voice.local_transcription.model",
        description: "Model identifier sent to the local server, or substituted for {model} in the command arguments. whisper-server ignores it; OpenAI-compatible servers require it.",
    },
    local_voice_command: LocalVoiceCommand {
        type: String,
        default: String::new(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::ALL,
        private: false,
        toml_path: "agents.voice.local_transcription.command",
        description: "Path to the transcriber binary used when the backend is `command`.",
    },
    local_voice_command_args: LocalVoiceCommandArgs {
        type: String,
        default: DEFAULT_COMMAND_ARGS.to_string(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::ALL,
        private: false,
        toml_path: "agents.voice.local_transcription.command_args",
        description: "Whitespace-separated arguments for the transcriber binary. {audio} is the recording's temporary path, {model} the model setting, and {language} the spoken language code (`auto` when unset).",
    },
]);

impl LocalVoiceSettings {
    pub fn backend(&self) -> LocalVoiceBackend {
        *self.local_voice_backend
    }

    pub fn endpoint(&self) -> &str {
        self.local_voice_endpoint.trim()
    }

    pub fn model(&self) -> &str {
        self.local_voice_model.trim()
    }

    pub fn command(&self) -> &str {
        self.local_voice_command.trim()
    }

    pub fn command_args(&self) -> &str {
        self.local_voice_command_args.trim()
    }
}

#[cfg(test)]
#[path = "local_voice_tests.rs"]
mod tests;
