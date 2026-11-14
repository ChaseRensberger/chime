//! Desktop output. Text is passed through stdin, never through a shell command.
use std::{
    io::Write,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use tauri::Manager;

fn run(program: &str, args: &[&str], text: Option<&str>) -> Result<(), String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("{program}: {e}"))?;
    let mut input = child.stdin.take().unwrap();
    let bytes = text.unwrap_or("").as_bytes().to_vec();
    let writer = thread::spawn(move || input.write_all(&bytes));
    let deadline = Instant::now() + Duration::from_secs(3);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                break if status.success() {
                    Ok(())
                } else {
                    Err(format!("{program} failed"))
                }
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                break Err(format!("{program} did not respond"));
            }
        }
    };
    let _ = child.wait();
    let written = writer
        .join()
        .map_err(|_| "Clipboard writer stopped".to_string())?;
    status?;
    written.map_err(|e| e.to_string())
}

pub fn copy(text: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        run(
            "wl-copy",
            &["--type", "text/plain;charset=utf-8"],
            Some(text),
        )
    }
    #[cfg(target_os = "macos")]
    {
        run("pbcopy", &[], Some(text))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = text;
        Err("Clipboard is not supported on this platform yet".into())
    }
}

#[cfg(any(target_os = "linux", test))]
fn paste_keys(terminal: bool) -> Vec<String> {
    // wtype assigns evdev codes from 1 in keysym insertion order. Reserve
    // codes 1–46 with releases only, so V lands on KEY_V (47) even when a
    // compositor interprets shortcuts through its own keyboard layout.
    let mut args = Vec::new();
    for index in 0..46 {
        args.extend(["-p".into(), format!("U{:04X}", 0x2460 + index)]);
    }
    args.extend(["-M".into(), "ctrl".into()]);
    if terminal {
        args.extend(["-M".into(), "shift".into()]);
    }
    args.extend(["-k".into(), "v".into()]);
    if terminal {
        args.extend(["-m".into(), "shift".into()]);
    }
    args.extend(["-m".into(), "ctrl".into()]);
    // GTK 3's reverse keymap lookup skips the final code. A trailing neutral
    // release keeps V discoverable as a shortcut rather than literal text.
    args.extend(["-p".into(), "VoidSymbol".into()]);
    args
}

pub fn paste(
    terminal: bool,
    generation: &std::sync::atomic::AtomicU64,
    id: u64,
) -> Result<(), String> {
    // Allow the trigger chord to be released before sending the paste chord.
    thread::sleep(Duration::from_millis(250));
    if generation.load(std::sync::atomic::Ordering::SeqCst) != id {
        return Err("Cancelled".into());
    }
    #[cfg(target_os = "linux")]
    {
        let keys = paste_keys(terminal);
        let args: Vec<&str> = keys.iter().map(String::as_str).collect();
        run("wtype", &args, None)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = terminal;
        Err("Automatic paste is currently implemented for Wayland with wtype".into())
    }
}

#[tauri::command]
pub async fn copy_result(app: tauri::AppHandle) -> Result<(), String> {
    let text = app
        .state::<crate::session::Sessions>()
        .last_text()
        .ok_or("No transcript to copy")?;
    tauri::async_runtime::spawn_blocking(move || copy(&text))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    #[test]
    fn paste_keymap_matches_evdev_and_keeps_v_before_the_final_code() {
        for terminal in [false, true] {
            let args = super::paste_keys(terminal);
            let mut keymap = Vec::new();
            let mut held = Vec::new();
            let mut presses = Vec::new();
            for pair in args.chunks_exact(2) {
                let (operation, key) = (pair[0].as_str(), pair[1].as_str());
                match operation {
                    "-p" | "-k" => {
                        if !keymap.contains(&key) {
                            keymap.push(key);
                        }
                        if operation == "-k" {
                            presses.push(key);
                            assert_eq!(
                                held,
                                if terminal {
                                    vec!["ctrl", "shift"]
                                } else {
                                    vec!["ctrl"]
                                }
                            );
                        }
                    }
                    "-M" => held.push(key),
                    "-m" => assert_eq!(held.pop(), Some(key)),
                    _ => panic!("Unexpected wtype operation"),
                }
            }
            assert_eq!(presses, ["v"], "placeholder keys must only be released");
            assert_eq!(keymap.iter().position(|key| *key == "v"), Some(46));
            assert_eq!(keymap.last(), Some(&"VoidSymbol"));
            assert!(held.is_empty(), "all modifiers must be released");
        }
    }

    #[test]
    fn cancelled_delivery_sends_no_keys() {
        let generation = std::sync::atomic::AtomicU64::new(2);
        assert_eq!(
            super::paste(false, &generation, 1).unwrap_err(),
            "Cancelled"
        );
    }
}
