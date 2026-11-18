#[derive(serde::Serialize)]
pub struct Permissions {
    microphone: &'static str,
    accessibility: &'static str,
    automation: &'static str,
    #[serde(rename = "appPath")]
    app_path: Option<String>,
}

#[cfg(target_os = "macos")]
extern "C" {
    fn chime_microphone_status() -> i32;
    fn chime_request_microphone();
    fn chime_accessibility_status() -> i32;
    fn chime_automation_status() -> i32;
}

#[tauri::command]
pub async fn permissions_status() -> Option<Permissions> {
    #[cfg(target_os = "macos")]
    unsafe {
        Some(Permissions {
            app_path: std::env::current_exe().ok().and_then(|exe| {
                exe.ancestors()
                    .find(|p| p.extension().is_some_and(|ext| ext == "app"))
                    .map(|p| p.display().to_string())
            }),
            microphone: match chime_microphone_status() {
                0 => "Not requested",
                3 => "Allowed",
                _ => "Needs access",
            },
            accessibility: match chime_accessibility_status() {
                1 => "Allowed",
                2 => "Check access",
                _ => "Needs access",
            },
            automation: match chime_automation_status() {
                0 => "Allowed",
                -1743 => "Needs access",
                -1744 => "Not requested",
                _ => "Not checked",
            },
        })
    }
    #[cfg(not(target_os = "macos"))]
    None
}

#[tauri::command]
pub async fn permissions_action(kind: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let pane = match kind.as_str() {
            "microphone" if unsafe { chime_microphone_status() } == 0 => {
                unsafe {
                    chime_request_microphone();
                }
                return Ok(());
            }
            "automation" if unsafe { chime_automation_status() } != -1743 => {
                let result = tauri::async_runtime::spawn_blocking(|| {
                    std::process::Command::new("/usr/bin/osascript")
                        .args([
                            "-e",
                            "tell application \"System Events\" to get UI elements enabled",
                        ])
                        .output()
                })
                .await
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
                if result.status.success() {
                    return Ok(());
                }
                "Privacy_Automation"
            }
            "microphone" => "Privacy_Microphone",
            "accessibility" => "Privacy_Accessibility",
            "automation" => "Privacy_Automation",
            _ => return Err("Unknown permission".into()),
        };
        let status = std::process::Command::new("/usr/bin/open")
            .arg(format!(
                "x-apple.systempreferences:com.apple.preference.security?{pane}"
            ))
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err("Could not open System Settings.".into())
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = kind;
        Err("Permission settings are managed by your desktop.".into())
    }
}
