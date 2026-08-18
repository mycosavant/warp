//! On-device voice transcription.
//!
//! Upstream's only [`Transcriber`] is `server::voice_transcriber::
//! ServerVoiceTranscriber`, which base64-encodes the recording and POSTs it to
//! `api.warp.dev`. That is true for *both* values of
//! `ai::voice::transcribe::Provider` — the enum selects Warp's upstream vendor,
//! not where inference happens — so selecting `OpenAI` instead of `Wispr` does
//! not keep audio on the machine.
//!
//! This transcriber sends the recording to something the user runs instead,
//! and it is **fail-closed**: when it is registered it is the only transcriber,
//! and a misconfiguration surfaces as a transcription error rather than
//! silently falling back to the server path. A fallback would defeat the point,
//! because the failure it papers over is precisely "audio went somewhere else".
//!
//! Two backends, both configured through `settings::LocalVoiceSettings`:
//!
//! * [`LocalVoiceBackend::Http`] posts `multipart/form-data` to a local server.
//!   whisper.cpp's `whisper-server` and the OpenAI-compatible servers
//!   (speaches, faster-whisper-server, LocalAI) agree on the request shape and
//!   on `{"text": ...}` for the response; they differ only in route, which is
//!   why the endpoint setting is a whole URL.
//! * [`LocalVoiceBackend::Command`] writes the recording to a private temporary
//!   file and runs a binary over it, reading the transcript from stdout.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use anyhow::{Context as _, anyhow};
use async_trait::async_trait;
use base64::Engine as _;
use parking_lot::RwLock;

use warpui::{ModelContext, SingletonEntity as _};

use crate::server::server_api::TranscribeError;
use crate::settings::{LocalVoiceBackend, LocalVoiceSettings};
use crate::voice::transcriber::{Transcriber, VoiceTranscriber};

/// Generous enough for a long recording on a CPU-only model, short enough that
/// a wedged server does not leave the input stuck in "Transcribing" forever.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

/// Guards against a transcriber binary that streams unbounded diagnostics on
/// failure; only the tail is ever shown to the user.
const MAX_ERROR_DETAIL_BYTES: usize = 2048;

/// whisper.cpp's spelling for "detect the spoken language".
const AUTO_LANGUAGE: &str = "auto";

/// A snapshot of `LocalVoiceSettings`, readable off the main thread.
///
/// Settings live in a main-thread singleton, but `Transcriber::transcribe` runs
/// on a background task with no context, so the values are mirrored here and
/// refreshed by an observer — see [`LocalTranscriber::config_handle`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LocalVoiceConfig {
    pub backend: LocalVoiceBackend,
    pub endpoint: String,
    pub model: String,
    pub command: String,
    pub command_args: String,
}

impl LocalVoiceConfig {
    pub fn from_settings(settings: &LocalVoiceSettings) -> Self {
        Self {
            backend: settings.backend(),
            endpoint: settings.endpoint().to_owned(),
            model: settings.model().to_owned(),
            command: settings.command().to_owned(),
            command_args: settings.command_args().to_owned(),
        }
    }
}

/// Builds the fork's voice transcriber, or `None` when fork policy is off.
///
/// Returning `None` rather than taking the upstream branch itself keeps the
/// call site in `lib.rs` a two-line seam with upstream's construction still
/// spelled out there, so an upstream change to it conflicts visibly.
///
/// The settings subscription exists because `Transcriber::transcribe` runs off
/// the main thread and cannot reach a context; without it, changing the
/// endpoint would need an app restart.
pub fn fork_voice_transcriber(
    ctx: &mut ModelContext<VoiceTranscriber>,
) -> Option<VoiceTranscriber> {
    if !crate::fork::local_voice_transcription_enabled() {
        return None;
    }

    let transcriber = LocalTranscriber::new(LocalVoiceConfig::from_settings(
        LocalVoiceSettings::as_ref(ctx),
    ));
    track_settings(transcriber.config_handle(), ctx);
    Some(VoiceTranscriber::new(Arc::new(transcriber)))
}

/// Republishes `LocalVoiceSettings` into the snapshot the transcriber reads.
///
/// Separate from [`fork_voice_transcriber`] only so a test can hold the same
/// handle and prove the subscription fires; settings groups emit a
/// `…ChangedEvent` rather than notifying observers, and getting that wrong
/// would show up as an endpoint change that silently needs a restart.
pub(crate) fn track_settings<T: warpui::Entity>(
    config: Arc<RwLock<LocalVoiceConfig>>,
    ctx: &mut ModelContext<T>,
) {
    ctx.subscribe_to_model(&LocalVoiceSettings::handle(ctx), move |_, _, _, ctx| {
        *config.write() = LocalVoiceConfig::from_settings(LocalVoiceSettings::as_ref(ctx));
    });
}

pub struct LocalTranscriber {
    config: Arc<RwLock<LocalVoiceConfig>>,
    client: http_client::Client,
}

impl LocalTranscriber {
    pub fn new(config: LocalVoiceConfig) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            client: http_client::Client::new(),
        }
    }

    /// Handle used to publish settings changes into the transcriber.
    ///
    /// Returned rather than exposing a setter so the registration site can hold
    /// it after the transcriber has been erased to `Arc<dyn Transcriber>`.
    pub fn config_handle(&self) -> Arc<RwLock<LocalVoiceConfig>> {
        self.config.clone()
    }

    async fn transcribe_over_http(
        &self,
        config: &LocalVoiceConfig,
        wav: Vec<u8>,
        language: Option<String>,
    ) -> anyhow::Result<String> {
        if config.endpoint.is_empty() {
            return Err(anyhow!(
                "No local transcription endpoint is configured. Set \
                 `agents.voice.local_transcription.endpoint` in settings.toml — \
                 whisper.cpp's whisper-server listens on \
                 http://127.0.0.1:8080/inference by default."
            ));
        }

        let mut form = reqwest::multipart::Form::new().part(
            "file",
            reqwest::multipart::Part::bytes(wav)
                .file_name("voice.wav")
                .mime_str("audio/wav")
                .context("audio/wav is a valid MIME type")?,
        );
        // whisper-server defaults to `json` already, but OpenAI-compatible
        // servers default to `text`, and this path only reads `{"text": ...}`.
        form = form.text("response_format", "json");
        if !config.model.is_empty() {
            form = form.text("model", config.model.clone());
        }
        if let Some(language) = language {
            form = form.text("language", language);
        }

        let response = self
            .client
            .post(config.endpoint.as_str())
            .multipart(form)
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await
            .with_context(|| {
                format!(
                    "Could not reach the local transcription server at {}. Is it running?",
                    config.endpoint
                )
            })?;

        let status = response.status();
        let body = response
            .text()
            .await
            .context("Failed to read the local transcription server's response")?;
        if !status.is_success() {
            return Err(anyhow!(
                "Local transcription server returned {status}: {}",
                truncate_detail(&body)
            ));
        }

        parse_transcription_response(&body)
    }

    fn transcribe_with_command(
        config: &LocalVoiceConfig,
        wav: Vec<u8>,
        language: Option<String>,
    ) -> anyhow::Result<String> {
        if config.command.is_empty() {
            return Err(anyhow!(
                "The local transcription backend is set to `command` but \
                 `agents.voice.local_transcription.command` is empty."
            ));
        }
        let args = build_command_args(
            &config.command_args,
            config.model.as_str(),
            language.as_deref(),
        )?;

        let recording = TempRecording::write(&wav)?;
        let args = substitute_audio_path(args, recording.path());

        // `command::blocking` rather than `std::process`: it sets
        // CREATE_NO_WINDOW, so transcribing does not flash a console window on
        // Windows every time the user speaks.
        let output = command::blocking::Command::new(&config.command)
            .args(&args)
            .output()
            .with_context(|| format!("Failed to run local transcriber `{}`", config.command))?;

        if !output.status.success() {
            return Err(anyhow!(
                "Local transcriber `{}` exited with {}: {}",
                config.command,
                output.status,
                truncate_detail(&String::from_utf8_lossy(&output.stderr))
            ));
        }

        let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if text.is_empty() {
            return Err(anyhow!(
                "Local transcriber `{}` produced no transcript on stdout. \
                 Check that its arguments write the transcript there — \
                 whisper-cli needs `--no-timestamps`.",
                config.command
            ));
        }
        Ok(text)
    }
}

#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
impl Transcriber for LocalTranscriber {
    async fn transcribe(
        &self,
        wav_base64: String,
        language: Option<String>,
    ) -> Result<String, TranscribeError> {
        let config = self.config.read().clone();
        let wav = base64::engine::general_purpose::STANDARD
            .decode(wav_base64)
            .context("Voice recording was not valid base64")
            .map_err(TranscribeError::Other)?;

        let result = match config.backend {
            LocalVoiceBackend::Http => self.transcribe_over_http(&config, wav, language).await,
            // `Command::output` blocks, and this future runs on a shared
            // executor, so the process gets its own thread. Transcription is
            // user-initiated and infrequent enough that a thread per call is
            // cheaper than depending on a particular async runtime being
            // installed under us.
            LocalVoiceBackend::Command => {
                run_on_thread(move || Self::transcribe_with_command(&config, wav, language)).await
            }
        };

        result.map_err(TranscribeError::Other)
    }
}

/// Runs blocking work on a dedicated thread and awaits its result.
async fn run_on_thread<T, F>(work: F) -> anyhow::Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> anyhow::Result<T> + Send + 'static,
{
    let (sender, receiver) = futures::channel::oneshot::channel();
    std::thread::Builder::new()
        .name("local-transcriber".to_owned())
        .spawn(move || {
            // The receiver is dropped only if the transcription was cancelled,
            // in which case nobody wants the result.
            let _ = sender.send(work());
        })
        .context("Failed to spawn the local transcription thread")?;
    receiver
        .await
        .context("Local transcription thread ended without a result")?
}

/// Extracts the transcript from a transcription server's JSON response.
///
/// whisper.cpp's `whisper-server` and the OpenAI-compatible servers both return
/// `{"text": "..."}`, with whisper.cpp including the leading space and trailing
/// newline that the decoder emits.
fn parse_transcription_response(body: &str) -> anyhow::Result<String> {
    let value: serde_json::Value = serde_json::from_str(body).with_context(|| {
        format!(
            "Local transcription server returned a non-JSON response: {}",
            truncate_detail(body)
        )
    })?;
    // Some servers report failures as a 200 carrying an `error` field.
    if let Some(error) = value.get("error") {
        let message = error
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| error.to_string());
        return Err(anyhow!("Local transcription server reported: {message}"));
    }
    value
        .get("text")
        .and_then(serde_json::Value::as_str)
        .map(|text| text.trim().to_owned())
        .ok_or_else(|| {
            anyhow!(
                "Local transcription server's response had no `text` field: {}",
                truncate_detail(body)
            )
        })
}

/// Splits the configured argument template and substitutes `{model}` and
/// `{language}`.
///
/// Splitting happens *before* substitution so that a value containing spaces —
/// a model path under `C:\Program Files`, say — stays one argument.
/// `{audio}` is left for [`substitute_audio_path`], which runs once the
/// temporary file exists.
pub(crate) fn build_command_args(
    template: &str,
    model: &str,
    language: Option<&str>,
) -> anyhow::Result<Vec<String>> {
    let language = language.unwrap_or(AUTO_LANGUAGE);
    if template.contains("{model}") && model.is_empty() {
        return Err(anyhow!(
            "The local transcriber's arguments reference {{model}} but \
             `agents.voice.local_transcription.model` is empty. For whisper.cpp \
             this is the path to a ggml model file."
        ));
    }
    let args: Vec<String> = template
        .split_whitespace()
        .map(|token| {
            token
                .replace("{model}", model)
                .replace("{language}", language)
        })
        .collect();
    if !args.iter().any(|arg| arg.contains("{audio}")) {
        return Err(anyhow!(
            "The local transcriber's arguments must include {{audio}}, which is \
             replaced with the recording's path."
        ));
    }
    Ok(args)
}

fn substitute_audio_path(args: Vec<String>, audio: &Path) -> Vec<String> {
    let audio = audio.to_string_lossy();
    args.into_iter()
        .map(|arg| arg.replace("{audio}", audio.as_ref()))
        .collect()
}

fn truncate_detail(detail: &str) -> String {
    let detail = detail.trim();
    if detail.len() <= MAX_ERROR_DETAIL_BYTES {
        return detail.to_owned();
    }
    // Keep the tail: a failing transcriber's last lines say why, the first
    // lines are model-loading banners.
    let mut start = detail.len() - MAX_ERROR_DETAIL_BYTES;
    while !detail.is_char_boundary(start) {
        start += 1;
    }
    format!("…{}", &detail[start..])
}

/// A recording on disk, removed when this value is dropped.
///
/// The file holds the user's voice, so on Unix it is created 0600 rather than
/// inheriting a world-readable `/tmp` umask. Windows temporary directories are
/// already per-user.
struct TempRecording {
    path: PathBuf,
}

impl TempRecording {
    fn write(wav: &[u8]) -> anyhow::Result<Self> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "warp-voice-{}-{}.wav",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));

        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let mut file = options
            .open(&path)
            .with_context(|| format!("Failed to create {}", path.display()))?;
        file.write_all(wav)
            .and_then(|()| file.flush())
            .with_context(|| format!("Failed to write {}", path.display()))?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempRecording {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_file(&self.path) {
            log::warn!(
                "Failed to remove voice recording {}: {error}",
                self.path.display()
            );
        }
    }
}

#[cfg(test)]
#[path = "local_transcriber_tests.rs"]
mod tests;
