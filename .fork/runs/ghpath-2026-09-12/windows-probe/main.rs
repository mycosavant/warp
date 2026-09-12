// Does setting PATH on a child change which binary Windows runs?
//
// Two cases, the same two measured on Linux 2026-09-12:
//   hostname       -- exists on the PARENT's PATH (System32) and, as a fake,
//                     first on the CHILD's PATH.
//   warp-only-here -- exists ONLY as a fake on the CHILD's PATH.
//
// On Linux: the parent's copy won the first, and the child's PATH was used for
// the second (a fallback). If Windows has no fallback, case two must fail to
// spawn.
use std::process::Command;

fn main() {
    let dir = std::path::PathBuf::from(r"C:\dev\pathprobe\fakebin");
    let parent_path = std::env::var("PATH").unwrap_or_default();
    let child_path = format!("{};{}", dir.display(), parent_path);

    // Negative control: no PATH set on the child at all. If the fakes still
    // run here, the probe is measuring something other than the child's PATH.
    for name in ["hostname", "warp-only-here"] {
        match Command::new(name).output() {
            Ok(out) => println!(
                "CONTROL (no child PATH) {name:<15} -> {}",
                String::from_utf8_lossy(&out.stdout).lines().next().unwrap_or("(no stdout)")
            ),
            Err(e) => println!("CONTROL (no child PATH) {name:<15} -> SPAWN FAILED: {e}"),
        }
    }

    for name in ["hostname", "warp-only-here"] {
        for with_cwd in [false, true] {
            let mut cmd = Command::new(name);
            cmd.env("PATH", &child_path);
            if with_cwd {
                cmd.current_dir(r"C:\dev");
            }
            let label = format!("{name:<15} cwd={with_cwd:<5}");
            match cmd.output() {
                Ok(out) => {
                    let s = String::from_utf8_lossy(&out.stdout);
                    println!("PROBE {label} -> {}", s.lines().next().unwrap_or("(no stdout)"));
                }
                Err(e) => println!("PROBE {label} -> SPAWN FAILED: {e}"),
            }
        }
    }
}
