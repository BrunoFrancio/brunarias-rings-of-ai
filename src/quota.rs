// Uma janela de cota, venha do Claude ou do Codex.

#[derive(Debug, PartialEq)]
pub struct Window {
    pub label: String,
    pub used: f32, // 0.0 a 1.0
    pub resets_at: Option<String>, // RFC 3339
}

impl Window {
    /// A janela já renovou depois da leitura: o uso atual é desconhecido.
    /// Regra herdada do widget do Dock: nunca presumir 0%.
    pub fn stale(&self, now: chrono::DateTime<chrono::Utc>) -> bool {
        self.resets_at
            .as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .is_some_and(|reset| reset <= now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> chrono::DateTime<chrono::Utc> {
        s.parse().unwrap()
    }

    fn window(resets_at: Option<&str>) -> Window {
        Window { label: "x".into(), used: 0.5, resets_at: resets_at.map(String::from) }
    }

    #[test]
    fn reset_in_the_past_is_stale() {
        assert!(window(Some("2026-11-07T05:00:00Z")).stale(at("2026-11-07T05:00:01Z")));
    }

    #[test]
    fn reset_in_the_future_is_fresh() {
        assert!(!window(Some("2026-11-07T05:00:00-03:00")).stale(at("2026-11-07T07:59:00Z")));
    }

    #[test]
    fn unknown_reset_is_not_stale() {
        assert!(!window(None).stale(at("2026-11-07T05:00:00Z")));
        assert!(!window(Some("ontem")).stale(at("2026-11-07T05:00:00Z")));
    }
}
