// Cota do Codex pelos arquivos de sessão que o próprio Codex grava.
// Cada uso grava um evento `token_count` com `rate_limits`. Sem rede e sem token.
use crate::quota::Window;
use serde_json::Value;

/// `rate_limits` do último evento que o traz, entre as linhas de uma sessão.
pub fn latest_rate_limits<'a>(lines: impl Iterator<Item = &'a str>) -> Option<Value> {
    lines
        .filter(|l| l.contains("rate_limits"))
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .map(|mut v| v["payload"]["rate_limits"].take())
        .filter(|rl| rl.is_object())
        .last()
}

/// Janelas a partir de `rate_limits`.
pub fn windows_from(rate_limits: &Value) -> Vec<Window> {
    ["primary", "secondary"]
        .iter()
        .filter_map(|key| {
            let q = &rate_limits[key];
            let label = match q["window_minutes"].as_i64() {
                Some(300) => "Codex · 5 h".to_string(),
                Some(10080) => "Codex · 7 dias".to_string(),
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

/// Janela que já renovou conta como sem uso, a pedido do Bruno: se o Codex tivesse
/// rodado depois disso, a sessão teria gravado uma cota nova. Uso em outra máquina não
/// aparece aqui. A próxima renovação só se sabe quando o Codex rodar de novo.
pub fn after_reset(windows: &[Window], now: chrono::DateTime<chrono::Utc>) -> Vec<Window> {
    windows
        .iter()
        .map(|w| if w.stale(now) { Window { label: w.label.clone(), used: 0.0, resets_at: None } } else { w.clone() })
        .collect()
}

/// Lê a sessão mais recente do Codex deste Windows que tenha cota gravada.
/// O Codex do WSL parou em outubro de 2026 e fica de fora.
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
        let Ok(text) = std::fs::read_to_string(path) else { continue };
        if let Some(rl) = latest_rate_limits(text.lines()) {
            let windows = windows_from(&rl);
            if !windows.is_empty() {
                return Ok(windows);
            }
        }
    }
    Err("Codex: nenhuma cota gravada nas sessões recentes".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &str = r#"{"timestamp":"2026-11-07T10:00:00Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"primary":{"used_percent":50.0,"window_minutes":300,"resets_at":1794069176}}}}"#;
    const NEW: &str = r#"{"timestamp":"2026-11-07T11:34:13Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"limit_id":"codex","primary":{"used_percent":1.0,"window_minutes":300,"resets_at":1794069176},"secondary":{"used_percent":9.0,"window_minutes":10080,"resets_at":1794572524},"credits":{"has_credits":false}}}}"#;

    #[test]
    fn latest_event_wins_and_noise_is_ignored() {
        let lines = [OLD, "não é json", r#"{"type":"response_item","payload":{"type":"message"}}"#, NEW, r#"{"payload":{"type":"token_count","rate_limits":null}}"#];
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
                Window { label: "Codex · 7 dias".into(), used: 0.09, resets_at: Some("2026-11-13T12:22:04+00:00".into()) },
            ]
        );
    }

    #[test]
    fn renewed_window_counts_as_unused() {
        let now = "2026-11-09T17:00:00Z".parse().unwrap();
        let week = Window { label: "Codex · 7 dias".into(), used: 0.09, resets_at: Some("2026-11-13T12:22:04+00:00".into()) };
        let windows = [Window { label: "Codex · 5 h".into(), used: 0.4, resets_at: Some("2026-11-09T16:00:00+00:00".into()) }, week.clone()];
        assert_eq!(after_reset(&windows, now), vec![Window { label: "Codex · 5 h".into(), used: 0.0, resets_at: None }, week]);
    }

    #[test]
    fn other_window_size_keeps_minutes() {
        let rl: Value = serde_json::from_str(r#"{"primary":{"used_percent":20,"window_minutes":60}}"#).unwrap();
        assert_eq!(windows_from(&rl), vec![Window { label: "Codex · 60 min".into(), used: 0.2, resets_at: None }]);
    }
}
