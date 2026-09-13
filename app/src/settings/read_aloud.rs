//! Fork-local settings for reading agent replies aloud.
//!
//! Warp speaks through a command the user names and holds no engine of its
//! own. The reply is written to the command's stdin as Markdown; stripping
//! code blocks and tables is the command's job, because what should be said
//! depends on the voice doing the saying. See `voice::read_aloud` for the
//! consumer and `fork::read_aloud_enabled` for the gate.

use settings::macros::define_settings_group;
use settings::{SupportedPlatforms, SyncToCloud};

define_settings_group!(ReadAloudSettings, settings: [
    read_aloud_command: ReadAloudCommand {
        type: String,
        default: String::new(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::ALL,
        private: false,
        toml_path: "agents.voice.read_aloud.command",
        description: "Path to a command that reads text on stdin aloud, such as pocket-speak from mycosavant/pocket-tts. Used by \"Read last agent reply aloud\".",
    },
    read_aloud_args: ReadAloudArgs {
        type: String,
        default: String::new(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::ALL,
        private: false,
        toml_path: "agents.voice.read_aloud.args",
        description: "Whitespace-separated arguments for the read-aloud command, for example `--voice alba`.",
    },
]);

impl ReadAloudSettings {
    pub fn command(&self) -> &str {
        self.read_aloud_command.trim()
    }

    pub fn args(&self) -> &str {
        self.read_aloud_args.trim()
    }
}
