// Cota do Claude pelo login do Claude Code de terminal.
// O endereço não é documentado: o mesmo que o /usage do CLI usa.
use serde_json::Value;

#[derive(Debug, PartialEq)]
pub struct Window {
    pub label: String,
    pub used: f32, // 0.0 a 1.0
    pub resets_at: Option<String>,
}

/// Token do arquivo de login, se ainda não venceu (com 1 minuto de folga).
pub fn usable_token(credentials: &str, now_ms: i64) -> Option<String> {
    let v: Value = serde_json::from_str(credentials).ok()?;
    let oauth = &v["claudeAiOauth"];
    if oauth["expiresAt"].as_i64()? <= now_ms + 60_000 {
        return None;
    }
    oauth["accessToken"].as_str().map(String::from)
}

/// Janelas de uso da resposta de /api/oauth/usage.
/// A resposta nova traz `limits`; a antiga, `five_hour` e `seven_day`.
pub fn parse_usage(body: &str) -> Vec<Window> {
    let Ok(v) = serde_json::from_str::<Value>(body) else {
        return Vec::new();
    };
    let window = |label: String, percent: Option<f64>, resets: &Value| {
        percent.map(|p| Window {
            label,
            used: (p / 100.0) as f32,
            resets_at: resets.as_str().map(String::from),
        })
    };
    if let Some(limits) = v["limits"].as_array() {
        return limits
            .iter()
            .filter_map(|l| {
                let label = match l["kind"].as_str()? {
                    "session" => "Claude · 5 h".to_string(),
                    "weekly_all" => "Claude · 7 dias".to_string(),
                    "weekly_scoped" => match l["scope"]["model"]["display_name"].as_str() {
                        Some(model) => format!("Claude · 7 dias {model}"),
                        None => "Claude · 7 dias".to_string(),
                    },
                    _ => return None,
                };
                window(label, l["percent"].as_f64(), &l["resets_at"])
            })
            .collect();
    }
    [("five_hour", "Claude · 5 h"), ("seven_day", "Claude · 7 dias")]
        .into_iter()
        .filter_map(|(key, label)| {
            let w = &v[key];
            window(label.into(), w["utilization"].as_f64(), &w["resets_at"])
        })
        .collect()
}

fn home() -> std::path::PathBuf {
    std::env::var_os("USERPROFILE").unwrap_or_default().into()
}

fn now_ms() -> i64 {
    let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
    since.map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn current_token() -> Option<String> {
    let text = std::fs::read_to_string(home().join(".claude/.credentials.json")).ok()?;
    usable_token(&text, now_ms())
}

/// O token só se renova quando o próprio CLI roda. `claude update` faz isso
/// sem gastar cota, como no Usage Monitor for Claude.
fn renew_login() {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;
    let mut cmd = std::process::Command::new(home().join(".local/bin/claude.exe"));
    cmd.arg("update").creation_flags(CREATE_NO_WINDOW);
    // Rodando de dentro de outra sessão do Claude, essas variáveis mudam o login.
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("CLAUDE") {
            cmd.env_remove(key);
        }
    }
    let _ = cmd.output();
}

enum Fetch {
    Ok(String),
    LoginRejected,
    Failed(String),
}

fn fetch(token: &str) -> Fetch {
    let response = ureq::get("https://api.anthropic.com/api/oauth/usage")
        .header("Authorization", &format!("Bearer {token}"))
        .header("anthropic-beta", "oauth-2025-04-20")
        .header("User-Agent", "claude-cli (external, cli)")
        .call();
    match response {
        Ok(mut r) => match r.body_mut().read_to_string() {
            Ok(body) => Fetch::Ok(body),
            Err(e) => Fetch::Failed(e.to_string()),
        },
        Err(ureq::Error::StatusCode(401)) => Fetch::LoginRejected,
        Err(e) => Fetch::Failed(e.to_string()),
    }
}

/// Lê a cota agora. Renova o login uma vez se ele venceu ou foi recusado.
pub fn read_windows() -> Result<Vec<Window>, String> {
    let mut renewed = false;
    loop {
        let token = match current_token() {
            Some(t) => t,
            None if !renewed => {
                renew_login();
                renewed = true;
                continue;
            }
            None => return Err("login do Claude Code vencido: rode `claude auth login`".into()),
        };
        match fetch(&token) {
            Fetch::Ok(body) => {
                let windows = parse_usage(&body);
                return if windows.is_empty() {
                    Err("a resposta da cota veio num formato desconhecido".into())
                } else {
                    Ok(windows)
                };
            }
            Fetch::LoginRejected if !renewed => {
                renew_login();
                renewed = true;
            }
            Fetch::LoginRejected => return Err("o login foi recusado mesmo depois de renovar".into()),
            Fetch::Failed(e) => return Err(format!("falha ao consultar a cota: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_800_000_000_000;

    fn creds(expires_at: i64) -> String {
        format!(r#"{{"claudeAiOauth":{{"accessToken":"tok-fake","refreshToken":"r","expiresAt":{expires_at}}}}}"#)
    }

    #[test]
    fn token_valid_is_returned() {
        assert_eq!(usable_token(&creds(NOW + 3_600_000), NOW), Some("tok-fake".into()));
    }

    #[test]
    fn token_expired_or_about_to_is_refused() {
        assert_eq!(usable_token(&creds(NOW - 1), NOW), None);
        assert_eq!(usable_token(&creds(NOW + 30_000), NOW), None);
    }

    #[test]
    fn token_missing_or_broken_is_refused() {
        assert_eq!(usable_token("{}", NOW), None);
        assert_eq!(usable_token("not json", NOW), None);
    }

    #[test]
    fn limits_array_becomes_windows() {
        let body = r#"{"limits":[
            {"kind":"session","percent":42,"resets_at":"2026-11-07T05:00:00Z"},
            {"kind":"weekly_all","percent":9.5,"resets_at":"2026-11-13T09:00:00Z"},
            {"kind":"weekly_scoped","percent":30,"scope":{"model":{"display_name":"Opus"}}}
        ]}"#;
        assert_eq!(
            parse_usage(body),
            vec![
                Window { label: "Claude · 5 h".into(), used: 0.42, resets_at: Some("2026-11-07T05:00:00Z".into()) },
                Window { label: "Claude · 7 dias".into(), used: 0.095, resets_at: Some("2026-11-13T09:00:00Z".into()) },
                Window { label: "Claude · 7 dias Opus".into(), used: 0.30, resets_at: None },
            ]
        );
    }

    #[test]
    fn old_shape_with_five_hour_and_seven_day() {
        let body = r#"{"five_hour":{"utilization":88,"resets_at":"2026-11-07T05:00:00Z"},
                       "seven_day":{"utilization":12,"resets_at":null},
                       "seven_day_opus":null}"#;
        assert_eq!(
            parse_usage(body),
            vec![
                Window { label: "Claude · 5 h".into(), used: 0.88, resets_at: Some("2026-11-07T05:00:00Z".into()) },
                Window { label: "Claude · 7 dias".into(), used: 0.12, resets_at: None },
            ]
        );
    }

    #[test]
    fn unknown_or_broken_body_gives_nothing() {
        assert!(parse_usage("{}").is_empty());
        assert!(parse_usage("<html>").is_empty());
    }
}
