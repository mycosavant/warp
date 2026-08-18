use std::fs;

use settings::Setting as _;
use warpui::App;

use super::*;
use crate::test_util::settings::initialize_settings_for_tests;

/// Under fork policy this is the *only* transcriber installed, so the app can
/// never fall back to the path that POSTs audio to `api.warp.dev`.
#[test]
fn fork_policy_installs_a_local_transcriber() {
    App::test((), |mut app| async move {
        initialize_settings_for_tests(&mut app);
        let transcriber = app.add_singleton_model(|ctx| {
            fork_voice_transcriber(ctx).expect("fork policy installs a local transcriber")
        });
        app.read(|ctx| {
            assert!(
                transcriber.as_ref(ctx).transcriber().is_some(),
                "an installed transcriber with no backend would disable voice input"
            );
        });
    });
}

/// `transcribe` runs off the main thread against a snapshot, so an edit to the
/// endpoint has to be pushed into that snapshot. Without this the setting would
/// appear to work but only take effect after a restart.
#[test]
fn a_settings_change_reaches_the_transcriber_without_a_restart() {
    App::test((), |mut app| async move {
        initialize_settings_for_tests(&mut app);
        let config = Arc::new(RwLock::new(LocalVoiceConfig::default()));
        let tracked = config.clone();
        app.add_singleton_model(move |ctx| {
            track_settings(tracked, ctx);
            VoiceTranscriber::new(Arc::new(LocalTranscriber::new(LocalVoiceConfig::default())))
        });

        app.update(|ctx| {
            LocalVoiceSettings::handle(ctx).update(ctx, |settings, ctx| {
                settings
                    .local_voice_endpoint
                    .set_value("http://127.0.0.1:9999/inference".to_owned(), ctx)
                    .unwrap();
                settings
                    .local_voice_command
                    .set_value("/usr/bin/whisper-cli".to_owned(), ctx)
                    .unwrap();
            });
        });

        let config = config.read().clone();
        assert_eq!(config.endpoint, "http://127.0.0.1:9999/inference");
        assert_eq!(config.command, "/usr/bin/whisper-cli");
    });
}

/// Exactly what whisper.cpp's `whisper-server` returns from `/inference`,
/// captured from a live run: leading space, trailing newline.
const WHISPER_CPP_RESPONSE: &str = r#"{"text":" List the files in this directory.\n"}"#;

#[test]
fn whisper_cpp_response_is_parsed_and_trimmed() {
    assert_eq!(
        parse_transcription_response(WHISPER_CPP_RESPONSE).unwrap(),
        "List the files in this directory."
    );
}

/// OpenAI-compatible servers return the same `text` key, which is what makes
/// one HTTP backend enough for both.
#[test]
fn openai_shaped_response_is_parsed() {
    let body = r#"{"task":"transcribe","language":"english","duration":1.7,"text":"hello there"}"#;
    assert_eq!(parse_transcription_response(body).unwrap(), "hello there");
}

/// A 200 carrying an `error` field must not be reported as a successful empty
/// transcription.
#[test]
fn an_error_field_is_reported_even_on_success_status() {
    let error = parse_transcription_response(r#"{"error":"model not loaded"}"#).unwrap_err();
    assert!(error.to_string().contains("model not loaded"));
}

#[test]
fn a_response_without_text_is_an_error() {
    let error = parse_transcription_response(r#"{"segments":[]}"#).unwrap_err();
    assert!(error.to_string().contains("no `text` field"));
}

#[test]
fn a_non_json_response_is_an_error_quoting_the_body() {
    // whisper-server answers a malformed request with bare text.
    let error = parse_transcription_response("Invalid request").unwrap_err();
    assert!(error.to_string().contains("Invalid request"));
}

#[test]
fn command_args_substitute_model_and_language() {
    let args = build_command_args(
        "--model {model} --language {language} --file {audio}",
        "/models/ggml-base.bin",
        Some("es"),
    )
    .unwrap();
    assert_eq!(
        args,
        vec![
            "--model",
            "/models/ggml-base.bin",
            "--language",
            "es",
            "--file",
            "{audio}"
        ]
    );
}

/// An absent language means "detect it", which whisper.cpp spells `auto`.
/// Dropping the argument instead would leave a dangling `--language` flag.
#[test]
fn an_absent_language_becomes_auto() {
    let args = build_command_args("-l {language} -f {audio}", "m", None).unwrap();
    assert_eq!(args, vec!["-l", "auto", "-f", "{audio}"]);
}

/// Splitting before substitution is what lets a model path contain spaces.
#[test]
fn a_substituted_value_with_spaces_stays_one_argument() {
    let args = build_command_args(
        "--model {model} --file {audio}",
        r"C:\Program Files\whisper\ggml-base.bin",
        None,
    )
    .unwrap();
    assert_eq!(args.len(), 4);
    assert_eq!(args[1], r"C:\Program Files\whisper\ggml-base.bin");
}

#[test]
fn args_referencing_an_unset_model_are_rejected_with_guidance() {
    let error = build_command_args("--model {model} --file {audio}", "", None).unwrap_err();
    assert!(error.to_string().contains("ggml model file"));
}

/// Without `{audio}` the binary would be handed no recording and would either
/// hang on stdin or transcribe nothing.
#[test]
fn args_without_the_audio_placeholder_are_rejected() {
    let error = build_command_args("--language {language}", "m", None).unwrap_err();
    assert!(error.to_string().contains("{audio}"));
}

#[test]
fn the_audio_path_is_substituted_after_splitting() {
    let args = substitute_audio_path(
        vec!["-f".to_owned(), "{audio}".to_owned()],
        Path::new("/tmp/a b/voice.wav"),
    );
    assert_eq!(args, vec!["-f", "/tmp/a b/voice.wav"]);
}

#[test]
fn a_recording_is_written_then_removed_on_drop() {
    let wav = b"RIFF....WAVEfmt ".to_vec();
    let path = {
        let recording = TempRecording::write(&wav).unwrap();
        assert_eq!(fs::read(recording.path()).unwrap(), wav);
        recording.path().to_path_buf()
    };
    assert!(
        !path.exists(),
        "the recording must not outlive the transcription"
    );
}

/// The recording is the user's voice; `/tmp` is world-readable.
#[cfg(unix)]
#[test]
fn a_recording_is_private_to_the_user() {
    use std::os::unix::fs::PermissionsExt as _;

    let recording = TempRecording::write(b"audio").unwrap();
    let mode = fs::metadata(recording.path()).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600, "unexpected mode {:o}", mode & 0o777);
}

#[test]
fn error_detail_keeps_the_tail_and_is_bounded() {
    let detail = "x".repeat(MAX_ERROR_DETAIL_BYTES * 2) + "the actual failure";
    let truncated = truncate_detail(&detail);
    assert!(truncated.ends_with("the actual failure"));
    assert!(truncated.len() <= MAX_ERROR_DETAIL_BYTES + '…'.len_utf8());
}

#[test]
fn short_error_detail_is_passed_through_unchanged() {
    assert_eq!(truncate_detail("  boom \n"), "boom");
}

#[test]
fn truncation_does_not_split_a_multibyte_character() {
    let detail = "é".repeat(MAX_ERROR_DETAIL_BYTES);
    // Panics on a non-boundary slice, so reaching the assertion is the test.
    assert!(truncate_detail(&detail).starts_with('…'));
}

fn config_for_command(command: &str, args: &str) -> LocalVoiceConfig {
    LocalVoiceConfig {
        backend: LocalVoiceBackend::Command,
        endpoint: String::new(),
        model: String::new(),
        command: command.to_owned(),
        command_args: args.to_owned(),
    }
}

#[test]
fn the_command_backend_reports_a_missing_command() {
    let error =
        LocalTranscriber::transcribe_with_command(&config_for_command("", "{audio}"), vec![], None)
            .unwrap_err();
    assert!(error.to_string().contains("command` is empty"));
}

#[test]
fn the_command_backend_reports_a_binary_that_cannot_be_run() {
    let config = config_for_command("warp-no-such-transcriber", "{audio}");
    let error =
        LocalTranscriber::transcribe_with_command(&config, b"audio".to_vec(), None).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Failed to run local transcriber")
    );
}

/// Writes an executable script and returns its path, kept alive by the caller.
#[cfg(unix)]
fn write_script(name: &str, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;

    let path = std::env::temp_dir().join(name);
    fs::write(&path, body).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    path
}

/// Exercises the whole command path: the recording reaches disk intact, its
/// path arrives as a single argument, and stdout becomes the transcript.
#[cfg(unix)]
#[test]
fn the_command_backend_passes_the_recording_and_reads_stdout() {
    let script = write_script(
        "warp-voice-test-size.sh",
        "#!/bin/sh\nprintf 'bytes:%s' \"$(wc -c < \"$1\" | tr -d ' ')\"\n",
    );
    let wav = vec![7u8; 1234];
    let transcript = LocalTranscriber::transcribe_with_command(
        &config_for_command(&script.to_string_lossy(), "{audio}"),
        wav.clone(),
        None,
    )
    .unwrap();
    assert_eq!(transcript, format!("bytes:{}", wav.len()));
    fs::remove_file(script).ok();
}

#[cfg(unix)]
#[test]
fn a_failing_command_surfaces_its_stderr() {
    let script = write_script(
        "warp-voice-test-fail.sh",
        "#!/bin/sh\necho 'error: failed to load model' >&2\nexit 3\n",
    );
    let error = LocalTranscriber::transcribe_with_command(
        &config_for_command(&script.to_string_lossy(), "{audio}"),
        b"audio".to_vec(),
        None,
    )
    .unwrap_err();
    let message = error.to_string();
    assert!(message.contains("failed to load model"), "{message}");
    fs::remove_file(script).ok();
}

/// A transcriber invoked without `--no-timestamps` writes its transcript to a
/// file instead of stdout, which would otherwise look like silence.
#[cfg(unix)]
#[test]
fn a_command_producing_no_stdout_explains_why() {
    let script = write_script("warp-voice-test-silent.sh", "#!/bin/sh\nexit 0\n");
    let error = LocalTranscriber::transcribe_with_command(
        &config_for_command(&script.to_string_lossy(), "{audio}"),
        b"audio".to_vec(),
        None,
    )
    .unwrap_err();
    assert!(error.to_string().contains("--no-timestamps"));
    fs::remove_file(script).ok();
}

#[cfg(windows)]
#[test]
fn the_command_backend_passes_the_recording_and_reads_stdout() {
    let path = std::env::temp_dir().join("warp-voice-test.cmd");
    // `%~z1` is the size of the file named by the first argument, so this
    // checks the recording reached disk and its path arrived as one argument.
    fs::write(&path, "@echo off\r\necho bytes:%~z1\r\n").unwrap();
    let wav = vec![7u8; 1234];
    let transcript = LocalTranscriber::transcribe_with_command(
        &config_for_command(&path.to_string_lossy(), "{audio}"),
        wav.clone(),
        None,
    )
    .unwrap();
    assert_eq!(transcript, format!("bytes:{}", wav.len()));
    fs::remove_file(path).ok();
}

/// End-to-end against a real transcription server, which is the only thing
/// that proves the multipart request is shaped the way whisper.cpp expects —
/// the parser tests above would pass just as happily against a request the
/// server rejects.
///
/// Ignored by default because it needs a server. Run it with:
///
/// ```text
/// WARP_VOICE_TEST_ENDPOINT=http://127.0.0.1:8080/inference \
/// WARP_VOICE_TEST_WAV=/path/to/16khz-mono.wav \
///   cargo test -p warp --lib transcribes_a_real_recording -- --ignored --nocapture
/// ```
#[test]
#[ignore = "requires a running local transcription server"]
fn transcribes_a_real_recording_over_http() {
    let endpoint = std::env::var("WARP_VOICE_TEST_ENDPOINT")
        .expect("set WARP_VOICE_TEST_ENDPOINT to the transcription endpoint");
    let wav = fs::read(
        std::env::var("WARP_VOICE_TEST_WAV")
            .expect("set WARP_VOICE_TEST_WAV to a 16kHz mono wav file"),
    )
    .expect("the test recording is readable");

    let transcriber = LocalTranscriber::new(LocalVoiceConfig {
        backend: LocalVoiceBackend::Http,
        endpoint,
        model: std::env::var("WARP_VOICE_TEST_MODEL").unwrap_or_default(),
        ..LocalVoiceConfig::default()
    });
    let config = transcriber.config.read().clone();
    let transcript = futures::executor::block_on(transcriber.transcribe_over_http(
        &config,
        wav,
        Some("en".to_owned()),
    ))
    .expect("transcription succeeds");

    println!("transcript: {transcript:?}");
    assert!(!transcript.is_empty());
    assert!(
        !transcript.starts_with(' ') && !transcript.ends_with('\n'),
        "whisper.cpp's leading space and trailing newline must be trimmed"
    );
}

#[cfg(windows)]
#[test]
fn a_failing_command_surfaces_its_stderr() {
    let path = std::env::temp_dir().join("warp-voice-test-fail.cmd");
    fs::write(
        &path,
        "@echo off\r\necho error: failed to load model 1>&2\r\nexit /b 3\r\n",
    )
    .unwrap();
    let error = LocalTranscriber::transcribe_with_command(
        &config_for_command(&path.to_string_lossy(), "{audio}"),
        b"audio".to_vec(),
        None,
    )
    .unwrap_err();
    let message = error.to_string();
    assert!(message.contains("failed to load model"), "{message}");
    fs::remove_file(path).ok();
}
