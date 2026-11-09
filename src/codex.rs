// Reads token_count events with rate_limits from local Codex sessions without network access.
use crate::quota::Window;
use serde_json::Value;

/// Returns rate_limits from the last session event that includes it.
pub fn latest_rate_limits<'a>(lines: impl Iterator<Item = &'a str>) -> Option<Value> {
    lines
        .filter(|l| l.contains("rate_limits"))
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .map(|mut v| v["payload"]["rate_limits"].take())
        .filter(|rl| rl.is_object())
        .last()
}

/// Converts rate_limits into quota windows.
pub fn windows_from(rate_limits: &Value) -> Vec<Window> {
    ["primary", "secondary"]
        .iter()
        .filter_map(|key| {
            let q = &rate_limits[key];
            let label = match q["window_minutes"].as_i64() {
                Some(300) => "Codex · 5 h".to_string(),
                Some(10080) => "Codex · 7 days".to_string(),
                Some(m) => format!("Codex · {m} min"),
                None => return None,
            };
            let resets_at = q["resets_at"]
                .as_i64()
                .and_then(|s| chrono::DateTime::from_timestamp(s, 0))
                .map(|t| t.to_rfc3339());
            Some(Window { label, used: (q["used_percent"].as_f64()? / 100.0) as f32, resets_at })
        })
        .collect()
}

/// Treats a reset window as unused until Codex runs locally again.
/// Usage on other machines is absent from these logs, and the next reset is unknown.
pub fn after_reset(windows: &[Window], now: chrono::DateTime<chrono::Utc>) -> Vec<Window> {
    windows
        .iter()
        .map(|w| if w.stale(now) { Window { label: w.label.clone(), used: 0.0, resets_at: None } } else { w.clone() })
        .collect()
}

const TAIL: u64 = 1 << 20;

/// Reads up to max bytes from the end of a session. The parser skips a partial first line.
fn read_tail(path: &std::path::Path, max: u64) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(max))).ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// Reads the newest Windows Codex session with quota data. WSL sessions are excluded.
pub fn read_windows() -> Result<Vec<Window>, String> {
    let root = std::path::PathBuf::from(std::env::var_os("USERPROFILE").unwrap_or_default()).join(".codex/sessions");
    let mut files = Vec::new();
    let mut dirs = vec![root];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|e| e == "jsonl") {
                let modified = entry.metadata().and_then(|m| m.modified()).ok();
                files.push((modified, path));
            }
        }
    }
    files.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, path) in files.iter().take(8) {
        let Some(text) = read_tail(path, TAIL) else { continue };
        if let Some(rl) = latest_rate_limits(text.lines()) {
            let windows = windows_from(&rl);
            if !windows.is_empty() {
                return Ok(windows);
            }
        }
    }
    Err("Codex: no quota data in recent sessions".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &str = r#"{"timestamp":"2026-11-07T10:00:00Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"primary":{"used_percent":50.0,"window_minutes":300,"resets_at":1794069176}}}}"#;
    const NEW: &str = r#"{"timestamp":"2026-11-07T11:34:13Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"limit_id":"codex","primary":{"used_percent":1.0,"window_minutes":300,"resets_at":1794069176},"secondary":{"used_percent":9.0,"window_minutes":10080,"resets_at":1794572524},"credits":{"has_credits":false}}}}"#;

    #[test]
    fn latest_event_wins_and_noise_is_ignored() {
        let lines = [OLD, "not JSON", r#"{"type":"response_item","payload":{"type":"message"}}"#, NEW, r#"{"payload":{"type":"token_count","rate_limits":null}}"#];
        let rl = latest_rate_limits(lines.into_iter()).unwrap();
        assert_eq!(rl["primary"]["used_percent"], 1.0);
    }

    #[test]
    fn no_event_gives_none() {
        assert!(latest_rate_limits(["{}", "x"].into_iter()).is_none());
    }

    #[test]
    fn primary_and_secondary_become_windows() {
        let rl = latest_rate_limits([NEW].into_iter()).unwrap();
        assert_eq!(
            windows_from(&rl),
            vec![
                Window { label: "Codex · 5 h".into(), used: 0.01, resets_at: Some("2026-11-07T16:32:56+00:00".into()) },
                Window { label: "Codex · 7 days".into(), used: 0.09, resets_at: Some("2026-11-13T12:22:04+00:00".into()) },
            ]
        );
    }

    #[test]
    fn only_the_end_of_a_big_session_is_read() {
        let path = std::env::temp_dir().join(format!("codex-tail-{}.jsonl", std::process::id()));
        let filler = format!("{{\"payload\":\"{}\"}}\n", "x".repeat(10_000));
        std::fs::write(&path, format!("{OLD}\n{filler}{NEW}\n")).unwrap();
        let tail = read_tail(&path, 4096).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert!(tail.len() <= 4096);
        assert_eq!(latest_rate_limits(tail.lines()).unwrap()["primary"]["used_percent"], 1.0);
    }

    #[test]
    fn renewed_window_counts_as_unused() {
        let now = "2026-11-09T17:00:00Z".parse().unwrap();
        let week = Window { label: "Codex · 7 days".into(), used: 0.09, resets_at: Some("2026-11-13T12:22:04+00:00".into()) };
        let windows = [Window { label: "Codex · 5 h".into(), used: 0.4, resets_at: Some("2026-11-09T16:00:00+00:00".into()) }, week.clone()];
        assert_eq!(after_reset(&windows, now), vec![Window { label: "Codex · 5 h".into(), used: 0.0, resets_at: None }, week]);
    }

    #[test]
    fn other_window_size_keeps_minutes() {
        let rl: Value = serde_json::from_str(r#"{"primary":{"used_percent":20,"window_minutes":60}}"#).unwrap();
        assert_eq!(windows_from(&rl), vec![Window { label: "Codex · 60 min".into(), used: 0.2, resets_at: None }]);
    }
}
