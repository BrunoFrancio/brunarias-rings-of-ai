// Reads quota from the undocumented local server started by Antigravity.
use crate::quota::Window;
use serde_json::Value;

/// Finds the access token only for Antigravity. Related editors use a different app_data_dir.
pub fn csrf_token(command_line: &str) -> Option<String> {
    let args: Vec<&str> = command_line.split_whitespace().collect();
    let value = |flag: &str| {
        args.iter().enumerate().find_map(|(i, a)| match a.strip_prefix(flag) {
            Some("") => args.get(i + 1).map(|v| v.to_string()),
            Some(rest) => rest.strip_prefix('=').map(String::from),
            None => None,
        })
    };
    value("--app_data_dir").filter(|dir| dir.starts_with("antigravity"))?;
    value("--csrf_token")
}

/// Parses RetrieveUserQuotaSummary, ordering the five-hour window before the weekly window.
pub fn windows_from_summary(body: &str) -> Vec<Window> {
    let Ok(v) = serde_json::from_str::<Value>(body) else {
        return Vec::new();
    };
    let mut windows = Vec::new();
    for group in v["response"]["groups"].as_array().into_iter().flatten() {
        let name = match group["displayName"].as_str().unwrap_or("?") {
            "Gemini Models" => "Gemini",
            "Claude and GPT models" => "Claude and GPT",
            other => other,
        };
        let mut buckets: Vec<&Value> = group["buckets"].as_array().into_iter().flatten().collect();
        buckets.sort_by_key(|b| b["window"] != "5h");
        for b in buckets {
            let window = match b["window"].as_str() {
                Some("5h") => "5 h",
                Some("weekly") => "7 days",
                Some(other) => other,
                None => continue,
            };
            // Protobuf JSON omits zero fractions.
            let left = b["remainingFraction"].as_f64().unwrap_or(0.0);
            windows.push(Window {
                label: format!("Antigravity · {name} {window}"),
                used: (1.0 - left) as f32,
                resets_at: b["resetTime"].as_str().map(String::from),
            });
        }
    }
    windows
}

/// Reads quota while Antigravity is running.
pub fn read_windows() -> Result<Vec<Window>, String> {
    for (pid, line) in language_servers() {
        let Some(token) = csrf_token(&line) else { continue };
        for port in listening_ports(pid) {
            if let Some(body) = fetch(port, &token) {
                let windows = windows_from_summary(&body);
                if !windows.is_empty() {
                    return Ok(windows);
                }
            }
        }
    }
    Err("Open Antigravity to read its quota".into())
}

fn fetch(port: u16, token: &str) -> Option<String> {
    // The server uses its own certificate on 127.0.0.1.
    let tls = ureq::tls::TlsConfig::builder().disable_verification(true).build();
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_global(Some(std::time::Duration::from_secs(5)))
        .build()
        .into();
    let url = format!("https://127.0.0.1:{port}/exa.language_server_pb.LanguageServerService/RetrieveUserQuotaSummary");
    let mut response = agent
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Connect-Protocol-Version", "1")
        .header("X-Codeium-Csrf-Token", token)
        .send("{}")
        .ok()?;
    response.body_mut().read_to_string().ok()
}

/// Lists language_server processes and their command lines.
fn language_servers() -> Vec<(u32, String)> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::*;
    let mut found = Vec::new();
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return found;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snap, &mut entry) != 0;
        while ok {
            let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
            let exe = String::from_utf16_lossy(&entry.szExeFile[..len]);
            if exe.starts_with("language_server") {
                if let Some(line) = command_line(entry.th32ProcessID) {
                    found.push((entry.th32ProcessID, line));
                }
            }
            ok = Process32NextW(snap, &mut entry) != 0;
        }
        CloseHandle(snap);
    }
    found
}

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtQueryInformationProcess(process: isize, class: u32, info: *mut u8, len: u32, ret: *mut u32) -> i32;
}

// ProcessCommandLineInformation (60) returns the command line without reading process memory.
fn command_line(pid: u32) -> Option<String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut needed = 0u32;
        NtQueryInformationProcess(handle as isize, 60, std::ptr::null_mut(), 0, &mut needed);
        let mut buf = vec![0u64; (needed as usize).div_ceil(8).max(4)];
        let status = NtQueryInformationProcess(handle as isize, 60, buf.as_mut_ptr() as *mut u8, (buf.len() * 8) as u32, &mut needed);
        CloseHandle(handle);
        if status != 0 {
            return None;
        }
        // UNICODE_STRING holds byte length, capacity, and a pointer into the buffer.
        let bytes = *(buf.as_ptr() as *const u16) as usize;
        let text = *(buf.as_ptr() as *const u8).add(8).cast::<*const u16>();
        Some(String::from_utf16_lossy(std::slice::from_raw_parts(text, bytes / 2)))
    }
}

/// Returns the process's listening TCP ports on 127.0.0.1 or all addresses.
fn listening_ports(pid: u32) -> Vec<u16> {
    use windows_sys::Win32::NetworkManagement::IpHelper::{GetExtendedTcpTable, MIB_TCPROW_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER};
    const AF_INET: u32 = 2;
    let mut size = 0u32;
    unsafe {
        GetExtendedTcpTable(std::ptr::null_mut(), &mut size, 0, AF_INET, TCP_TABLE_OWNER_PID_LISTENER, 0);
        let mut buf = vec![0u32; (size as usize).div_ceil(4)];
        if GetExtendedTcpTable(buf.as_mut_ptr().cast(), &mut size, 0, AF_INET, TCP_TABLE_OWNER_PID_LISTENER, 0) != 0 {
            return Vec::new();
        }
        let count = buf[0] as usize;
        let rows = std::slice::from_raw_parts(buf.as_ptr().add(1).cast::<MIB_TCPROW_OWNER_PID>(), count);
        rows.iter()
            .filter(|r| r.dwOwningPid == pid && (r.dwLocalAddr == 0 || r.dwLocalAddr == u32::from_ne_bytes([127, 0, 0, 1])))
            .map(|r| u16::from_be(r.dwLocalPort as u16))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LS: &str = r"C:\x\resources\bin\language_server.exe --standalone --override_ide_name antigravity";

    #[test]
    fn token_comes_from_antigravity_command_line() {
        let line = format!("{LS} --https_server_port 0 --csrf_token abc-123 --app_data_dir antigravity --x=1");
        assert_eq!(csrf_token(&line), Some("abc-123".into()));
        let line = format!("{LS} --csrf_token=abc-123 --app_data_dir antigravity");
        assert_eq!(csrf_token(&line), Some("abc-123".into()));
    }

    #[test]
    fn other_codeium_servers_are_ignored() {
        assert_eq!(csrf_token(&format!("{LS} --csrf_token abc --app_data_dir windsurf")), None);
        assert_eq!(csrf_token(&format!("{LS} --app_data_dir antigravity")), None);
    }

    const BODY: &str = r#"{"response":{"groups":[
        {"displayName":"Gemini Models","buckets":[
            {"bucketId":"gemini-weekly","window":"weekly","remainingFraction":0.75,"resetTime":"2026-11-16T15:54:27Z"},
            {"bucketId":"gemini-5h","window":"5h","remainingFraction":1,"resetTime":"2026-11-09T20:54:27Z"}]},
        {"displayName":"Claude and GPT models","buckets":[
            {"bucketId":"3p-5h","window":"5h","resetTime":"2026-11-09T20:54:27Z"}]},
        {"displayName":"Gemini 4 Argon","buckets":[
            {"bucketId":"gemini-argon-wtu-weekly","window":"weekly","remainingFraction":0.912944,"resetTime":"2026-11-14T09:00:27Z"}]}
    ]}}"#;

    #[test]
    fn groups_become_windows_with_5h_first() {
        let w = windows_from_summary(BODY);
        let labels: Vec<&str> = w.iter().map(|w| w.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "Antigravity · Gemini 5 h",
                "Antigravity · Gemini 7 days",
                "Antigravity · Claude and GPT 5 h",
                "Antigravity · Gemini 4 Argon 7 days",
            ]
        );
        assert_eq!(w[0].used, 0.0);
        assert_eq!(w[1].used, 0.25);
        assert_eq!(w[1].resets_at.as_deref(), Some("2026-11-16T15:54:27Z"));
        assert!((w[3].used - 0.087056).abs() < 1e-6);
    }

    #[test]
    fn missing_fraction_means_nothing_left() {
        // Protobuf JSON omits zero values.
        let w = windows_from_summary(BODY);
        assert_eq!(w[2].label, "Antigravity · Claude and GPT 5 h");
        assert_eq!(w[2].used, 1.0);
    }

    #[test]
    fn broken_body_gives_nothing() {
        assert!(windows_from_summary("{}").is_empty());
        assert!(windows_from_summary("<html>").is_empty());
    }
}
