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
        if terminal {
            run(
                "wtype",
                &[
                    "-M", "ctrl", "-M", "shift", "-k", "v", "-m", "shift", "-m", "ctrl",
                ],
                None,
            )
        } else {
            run("wtype", &["-M", "ctrl", "-k", "v", "-m", "ctrl"], None)
        }
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
    fn cancelled_delivery_sends_no_keys() {
        let generation = std::sync::atomic::AtomicU64::new(2);
        assert_eq!(
            super::paste(false, &generation, 1).unwrap_err(),
            "Cancelled"
        );
    }
}
