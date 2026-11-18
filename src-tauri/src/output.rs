//! Desktop output. Text is passed through stdin, never through a shell command.
use std::{
    io::{Read, Write},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use tauri::Manager;

static CLIPBOARD: Mutex<()> = Mutex::new(());

fn helper(
    program: &str,
    args: &[&str],
    text: Option<&str>,
    limit: usize,
    timeout: Duration,
    valid: impl Fn() -> bool,
) -> Result<Vec<u8>, String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(if limit > 0 {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stderr(if program == "osascript" {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .spawn()
        .map_err(|e| format!("{program}: {e}"))?;
    let mut input = child.stdin.take().unwrap();
    let bytes = text.unwrap_or("").as_bytes().to_vec();
    let writer = thread::spawn(move || input.write_all(&bytes));
    let stdout = child.stdout.take().map(|out| {
        thread::spawn(move || {
            let mut bytes = Vec::new();
            out.take(limit as u64)
                .read_to_end(&mut bytes)
                .map(|_| bytes)
        })
    });
    let stderr = child.stderr.take().map(|err| {
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = err.take(4096).read_to_end(&mut bytes);
            String::from_utf8_lossy(&bytes).trim().to_owned()
        })
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                break if status.success() {
                    Ok(())
                } else {
                    Err(format!("{program} failed"))
                }
            }
            Ok(None) if !valid() => {
                let _ = child.kill();
                break Err("Cancelled".into());
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
    let detail = stderr.and_then(|t| t.join().ok()).unwrap_or_default();
    let bytes = stdout
        .map(|task| {
            task.join()
                .map_err(|_| "Clipboard reader stopped".to_owned())?
                .map_err(|e| e.to_string())
        })
        .transpose()?;
    status.map_err(|error| {
        if detail.is_empty() {
            error
        } else {
            format!("{error}: {detail}")
        }
    })?;
    written.map_err(|e| e.to_string())?;
    Ok(bytes.unwrap_or_default())
}

fn run(program: &str, args: &[&str], text: Option<&str>) -> Result<(), String> {
    helper(program, args, text, 0, Duration::from_secs(3), || true).map(|_| ())
}

pub fn copy(text: &str) -> Result<(), String> {
    let _guard = CLIPBOARD.lock().unwrap();
    write_clipboard(text)
}

fn write_clipboard(text: &str) -> Result<(), String> {
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

pub fn deliver(text: &str, terminal: bool, generation: &AtomicU64, id: u64) -> Result<(), String> {
    // Allow the trigger chord to be released before sending the paste chord.
    let valid = || generation.load(Ordering::SeqCst) == id;
    if !valid() {
        return Err("Cancelled".into());
    }
    #[cfg(target_os = "macos")]
    {
        let result = helper(
            "osascript",
            &[
                "-e",
                "tell application \"System Events\" to get UI elements enabled",
            ],
            None,
            80,
            Duration::from_secs(120),
            valid,
        )
        .map_err(|e| {
            format!("Allow Chime to control System Events in Privacy & Security → Automation: {e}")
        })?;
        if result.trim_ascii() != b"true" {
            return Err(
                "Allow Chime in Privacy & Security → Accessibility, then try again.".into(),
            );
        }
    }
    thread::sleep(Duration::from_millis(250));
    let _guard = CLIPBOARD.lock().unwrap();
    if !valid() {
        return Err("Cancelled".into());
    }
    write_clipboard(text)?;
    #[cfg(target_os = "linux")]
    let check = (
        "wl-paste",
        &["--no-newline", "--type", "text/plain;charset=utf-8"][..],
    );
    #[cfg(target_os = "macos")]
    let check = ("pbpaste", &[][..]);
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let current = helper(
            check.0,
            check.1,
            None,
            text.len() + 1,
            Duration::from_secs(3),
            valid,
        )?;
        paste_if_current(text, &current, valid, || send_keys(terminal))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = terminal;
        Err("Automatic paste unavailable.".into())
    }
}

fn paste_if_current(
    text: &str,
    current: &[u8],
    valid: impl FnOnce() -> bool,
    send: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    if !valid() {
        return Err("Cancelled".into());
    }
    if current != text.as_bytes() {
        return Err("Clipboard changed before paste.".into());
    }
    send()
}

fn send_keys(terminal: bool) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let keys = paste_keys(terminal);
        let args: Vec<&str> = keys.iter().map(String::as_str).collect();
        run("wtype", &args, None)
    }
    #[cfg(target_os = "macos")]
    {
        let _ = terminal;
        run(
            "osascript",
            &[
                "-e",
                "tell application \"System Events\" to keystroke \"v\" using command down",
            ],
            None,
        )
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
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
    fn clipboard_replaced_or_cancelled_cannot_trigger_a_paste() {
        let send = || -> Result<(), String> { panic!("must not send a paste chord") };
        assert!(super::paste_if_current("dictation", b"another copy", || true, send).is_err());
        assert!(super::paste_if_current("dictation", b"dictation", || false, send).is_err());
        assert!(super::paste_if_current("dictation", b"dictation", || true, || Ok(())).is_ok());
    }
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
            super::deliver("unused", false, &generation, 1).unwrap_err(),
            "Cancelled"
        );
    }
}
