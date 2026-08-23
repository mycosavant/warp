//! A CLI tool for manually testing computer use actions.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use computer_use::{
    Action, Key, MouseButton, Options, ScreenshotParams, ScreenshotRegion, Target, TargetedAction,
    Vector2I,
};

#[derive(Parser)]
#[command(name = "use_computer")]
#[command(about = "Manually test computer use actions")]
struct Cli {
    /// Experimental: target a specific window instead of the screen, without moving the real
    /// cursor. On macOS events are delivered to this process ID; on Linux X11 and on Windows
    /// delivery is addressed by `--window-id` and the pid is informational.
    ///
    /// Windows caveat: posted input reaches the window without touching the cursor, but the
    /// window must be the foreground one — a posted drag on an inactive window is inert.
    #[arg(long, global = true)]
    pid: Option<i32>,

    /// Experimental: the platform window id to target (a CGWindowID on macOS, an X window id on
    /// Linux, an HWND on Windows). Required when `--pid` is given. Use the `windows` subcommand
    /// to list window ids.
    #[arg(long, global = true)]
    window_id: Option<u32>,

    #[command(subcommand)]
    command: Command,
}

impl Cli {
    /// Resolves the per-action / screenshot target from the CLI flags. `--pid` plus
    /// `--window-id` selects a background window target; otherwise the legacy whole-screen
    /// target is used. `main` rejects lone flags up front, so a partial combination can never
    /// silently downgrade to screen targeting or produce a `0`-id window target.
    fn target(&self) -> Target {
        match (self.pid, self.window_id) {
            (Some(pid), Some(window_id)) => Target::Window { window_id, pid },
            (Some(_), None) | (None, Some(_)) | (None, None) => Target::Screen,
        }
    }
}

#[derive(Subcommand)]
enum Command {
    /// Perform a mouse click (mouse down + mouse up) at a position.
    Click {
        /// X coordinate.
        x: i32,
        /// Y coordinate.
        y: i32,
        /// Which mouse button to click.
        #[arg(short, long, default_value = "left")]
        button: Button,
    },
    /// Press at one point, move to another, and release: a drag.
    ///
    /// The intermediate moves are not decoration. A `Draggable` only enters its dragging
    /// state after the pointer passes a threshold, and drop previews and hover indices are
    /// recomputed from each move — so a single jump from start to end exercises neither.
    Drag {
        /// X coordinate to press at.
        x1: i32,
        /// Y coordinate to press at.
        y1: i32,
        /// X coordinate to release at.
        x2: i32,
        /// Y coordinate to release at.
        y2: i32,
        /// Which mouse button to hold down for the drag.
        #[arg(short, long, default_value = "left")]
        button: Button,
        /// Number of intermediate moves between the two points.
        #[arg(long, default_value_t = 20)]
        steps: u32,
        /// Milliseconds to wait after each move.
        #[arg(long, default_value_t = 16)]
        step_ms: u64,
        /// Save a PNG *before the button is released* — the only moment a drop preview,
        /// a detached tab or a drag ghost is on screen. A screenshot taken after the
        /// release shows the result, which is a different question.
        #[arg(long)]
        screenshot: Option<PathBuf>,
    },
    /// Type text using the keyboard.
    Text {
        /// The text to type.
        text: String,
    },
    /// Take a screenshot and save it to a file.
    Screenshot {
        /// Output file path (PNG format).
        output: PathBuf,
        /// Optional region to capture as "x1,y1,x2,y2" (top-left and bottom-right coordinates).
        /// If not specified, captures the full display.
        #[arg(short, long, value_parser = parse_region)]
        region: Option<(i32, i32, i32, i32)>,
    },
    /// Press a key (key down + key up).
    Keypress {
        /// The key to press. Can be a single character (e.g., "a") or a keycode (e.g., "0x24" for Return on macOS).
        key: String,
    },
    /// Experimental: list on-screen windows with their window number, owner PID, owner name,
    /// and bounds, to help identify the right target PID/window.
    Windows,
}

#[derive(Clone, ValueEnum)]
enum Button {
    Left,
    Right,
    Middle,
}

impl From<Button> for MouseButton {
    fn from(button: Button) -> Self {
        match button {
            Button::Left => MouseButton::Left,
            Button::Right => MouseButton::Right,
            Button::Middle => MouseButton::Middle,
        }
    }
}

/// Parses a region string "x1,y1,x2,y2" into a tuple of coordinates.
fn parse_region(s: &str) -> Result<(i32, i32, i32, i32), String> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 4 {
        return Err("Region must be specified as 'x1,y1,x2,y2'".to_string());
    }
    let x1 = parts[0]
        .trim()
        .parse::<i32>()
        .map_err(|_| format!("Invalid x1: {}", parts[0]))?;
    let y1 = parts[1]
        .trim()
        .parse::<i32>()
        .map_err(|_| format!("Invalid y1: {}", parts[1]))?;
    let x2 = parts[2]
        .trim()
        .parse::<i32>()
        .map_err(|_| format!("Invalid x2: {}", parts[2]))?;
    let y2 = parts[3]
        .trim()
        .parse::<i32>()
        .map_err(|_| format!("Invalid y2: {}", parts[3]))?;
    Ok((x1, y1, x2, y2))
}

// The binary exits by returning an `ExitCode` rather than calling `std::process::exit`, which
// would skip `Drop` implementations: on Linux X11 the actor owns a server-global input device
// pair that must be removed when the actor is dropped.
#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    // Window listing does not go through the actor's action model; handle it up front.
    if let Command::Windows = cli.command {
        return match computer_use::experimental_list_windows() {
            Ok(text) => {
                print!("{text}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("Error: {e}");
                ExitCode::FAILURE
            }
        };
    }

    // Window targeting needs both flags: the window id addresses the window, and a lone
    // `--window-id` must not silently downgrade to screen targeting (nor a lone `--pid`
    // produce the ambiguous `0` window-id sentinel).
    match (cli.pid.is_some(), cli.window_id.is_some()) {
        (true, false) => {
            eprintln!(
                "--window-id is required when --pid is given. Use the `windows` subcommand to \
                 list window ids."
            );
            return ExitCode::FAILURE;
        }
        (false, true) => {
            eprintln!(
                "--pid is required when --window-id is given (on Linux X11 the pid is \
                 informational, but both flags select window targeting together). Use the \
                 `windows` subcommand to list window ids and pids."
            );
            return ExitCode::FAILURE;
        }
        (true, true) | (false, false) => {}
    }

    let target = cli.target();

    // `after` is performed as a second batch, on the same actor, once the screenshot has been
    // taken. Only `drag` uses it, and it exists for one reason: the frame worth capturing is
    // the one with the button still down.
    let (actions, screenshot_params, output_path, after) = match cli.command {
        Command::Click { x, y, button } => {
            let pos = Vector2I::new(x, y);
            let button: MouseButton = button.into();
            (
                vec![
                    Action::MouseDown { button, at: pos },
                    Action::MouseUp { button },
                ],
                None,
                None,
                Vec::new(),
            )
        }
        Command::Drag {
            x1,
            y1,
            x2,
            y2,
            button,
            steps,
            step_ms,
            screenshot,
        } => {
            let button: MouseButton = button.into();
            let step = std::time::Duration::from_millis(step_ms);
            let mut actions = vec![Action::MouseDown {
                button,
                at: Vector2I::new(x1, y1),
            }];
            // `steps` is the number of moves *after* the press, so the loop runs 1..=steps and
            // the last one lands exactly on (x2, y2) with no rounding drift.
            for i in 1..=steps.max(1) {
                let t = i as f64 / steps.max(1) as f64;
                let x = x1 as f64 + (x2 - x1) as f64 * t;
                let y = y1 as f64 + (y2 - y1) as f64 * t;
                actions.push(Action::MouseMove {
                    to: Vector2I::new(x.round() as i32, y.round() as i32),
                });
                actions.push(Action::Wait(step));
            }
            // Let the target render at the final position before it is photographed. Without
            // this the capture races the frame that draws the preview.
            actions.push(Action::Wait(std::time::Duration::from_millis(250)));
            let screenshot_params = screenshot.as_ref().map(|_| ScreenshotParams {
                max_long_edge_px: None,
                max_total_px: None,
                region: None,
                target,
            });
            (
                actions,
                screenshot_params,
                screenshot,
                vec![Action::MouseUp { button }],
            )
        }
        Command::Text { text } => (vec![Action::TypeText { text }], None, None, Vec::new()),
        Command::Screenshot { output, region } => {
            let region = region.map(|(x1, y1, x2, y2)| ScreenshotRegion {
                top_left: Vector2I::new(x1, y1),
                bottom_right: Vector2I::new(x2, y2),
            });
            (
                vec![],
                Some(ScreenshotParams {
                    max_long_edge_px: None,
                    max_total_px: None,
                    region,
                    target,
                }),
                Some(output),
                Vec::new(),
            )
        }
        Command::Keypress { key } => {
            let key = match parse_key(&key) {
                Ok(key) => key,
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::FAILURE;
                }
            };
            (
                vec![Action::KeyDown { key: key.clone() }, Action::KeyUp { key }],
                None,
                None,
                Vec::new(),
            )
        }
        // Handled up front, above.
        Command::Windows => unreachable!(),
    };

    // Pair every action with the resolved target before handing off to the actor.
    let actions: Vec<TargetedAction> = actions
        .into_iter()
        .map(|action| TargetedAction { action, target })
        .collect();
    // The CLI is a developer tool for exercising window targeting, so background per-window
    // control is always enabled here.
    let options = Options {
        screenshot_params,
        background_enabled: true,
        pointer_sink: None,
    };

    let after: Vec<TargetedAction> = after
        .into_iter()
        .map(|action| TargetedAction { action, target })
        .collect();

    let mut actor = computer_use::create_actor();
    let outcome = actor.perform_actions(&actions, options).await;

    // Run the trailing batch whether or not the first one succeeded. On a drag it holds the
    // mouse-up, and a physical button left down after a failed drag is worse than the failure:
    // the desktop stays in a drag until someone clicks.
    let mut release_failed = None;
    if !after.is_empty() {
        let release_options = Options {
            screenshot_params: None,
            background_enabled: true,
            pointer_sink: None,
        };
        if let Err(e) = actor.perform_actions(&after, release_options).await {
            release_failed = Some(e);
        }
    }

    let mut code = match outcome {
        Ok(result) => {
            if let Some(pos) = result.cursor_position {
                println!("Cursor position: ({}, {})", pos.x(), pos.y());
            }
            let mut code = ExitCode::SUCCESS;
            if let Some(screenshot) = result.screenshot
                && let Some(path) = output_path
            {
                match std::fs::write(&path, &screenshot.data) {
                    Ok(()) => println!(
                        "Screenshot saved to {} ({}x{})",
                        path.display(),
                        screenshot.width,
                        screenshot.height
                    ),
                    Err(e) => {
                        eprintln!("Failed to write screenshot: {e}");
                        code = ExitCode::FAILURE;
                    }
                }
            }
            code
        }
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::FAILURE
        }
    };

    if let Some(e) = release_failed {
        eprintln!("Error releasing the mouse button after the drag: {e}");
        code = ExitCode::FAILURE;
    }
    code
}

/// Parses a key argument: a "0x"-prefixed platform keycode, or a single character.
fn parse_key(key: &str) -> Result<Key, String> {
    if key.starts_with("0x") || key.starts_with("0X") {
        let keycode =
            i32::from_str_radix(&key[2..], 16).map_err(|_| format!("Invalid keycode: {key}"))?;
        return Ok(Key::Keycode(keycode));
    }
    let mut chars = key.chars();
    let ch = chars
        .next()
        .ok_or_else(|| "Key cannot be empty".to_string())?;
    if chars.next().is_some() {
        return Err(format!("Key must be a single character, got: {key}"));
    }
    Ok(Key::Char(ch))
}
