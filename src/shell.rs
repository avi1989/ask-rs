// Rust
use std::{collections::HashMap, path::Path};
use sysinfo;

pub fn detect_shell_kind() -> String {
    // Heuristics from env
    let env: HashMap<_, _> = std::env::vars().collect();

    // PowerShell usually sets these
    if env.contains_key("POWERSHELL_DISTRIBUTION_CHANNEL")
        || env.contains_key("PSModulePath")
        || env.contains_key("PSExecutionPolicyPreference")
    {
        return "Powershell".to_string();
    }

    // POSIX-like hints
    if env.contains_key("SHELL")
        || env.contains_key("BASH_VERSION")
        || env.contains_key("ZSH_VERSION")
        || env.contains_key("FISH_VERSION")
    {
        return "POSIX".to_string()
    }

    // Windows cmd hint (treat as non-Posix)
    if cfg!(windows) {
        if let Ok(comspec) = std::env::var("ComSpec") {
            let name = Path::new(&comspec).file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.eq_ignore_ascii_case("cmd.exe") {
                return "Powershell".to_string(); // Non-POSIX; treat as not POSIX
            }
        }
    }

    // Fallback: parent process name
    match parent_process_name().as_deref() {
        Some("pwsh") | Some("powershell") | Some("powershell.exe") | Some("pwsh.exe") => "Powershell".to_string(),
        Some("bash") | Some("zsh") | Some("fish") | Some("sh") |
        Some("bash.exe") | Some("zsh.exe") | Some("fish.exe") | Some("sh.exe") => "Powershell".to_string(),
        _ => "POSIX".to_string(),
    }
}

// Minimal parent-process name getter (sysinfo crate recommended for portability)
fn parent_process_name() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        let stat = fs::read_to_string("/proc/self/stat").ok()?;
        let ppid: i32 = stat.split_whitespace().nth(3)?.parse().ok()?;
        let name = fs::read_to_string(format!("/proc/{}/comm", ppid)).ok()?;
        return Some(name.trim().to_string());
    }
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        use std::ffi::CStr;
        use libc::{proc_name, getppid};
        unsafe {
            let mut buf = [0u8; 1024];
            let ppid = getppid();
            // macOS doesn't have a stable /proc; use libproc:
            // proc_name gets current, so use parent via sysctl/kinfo_proc is more accurate.
            // For brevity, return Unknown here.
        }
        None
    }
    #[cfg(windows)]
    {
        // Use sysinfo for simplicity on Windows
        use sysinfo::{System};
        let s = System::new_all();
        let pid = std::process::id();
        let proc = s.process(sysinfo::Pid::from_u32(pid))?;
        let ppid = proc.parent()?;
        let parent = s.process(ppid)?;
        return Some(parent.name().to_string_lossy().to_string());
    }
    #[allow(unreachable_code)]
    None
}