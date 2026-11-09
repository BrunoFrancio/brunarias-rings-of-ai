// Uma janela de cota, venha do Claude ou do Codex.

#[derive(Debug, Clone, PartialEq)]
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

/// Quando a janela renova, do jeito que se lê de relance:
/// "em 42 min", "em 2 h 10", "em 3 h 04" ou, a partir de um dia, "qua 06:59" no fuso de `reset`.
pub fn renew_text(reset: chrono::DateTime<chrono::FixedOffset>, now: chrono::DateTime<chrono::Utc>) -> String {
    use chrono::Datelike;
    let minutes = (reset.with_timezone(&chrono::Utc) - now).num_minutes();
    match minutes {
        ..60 => format!("em {minutes} min"),
        60..1440 if minutes % 60 == 0 => format!("em {} h", minutes / 60),
        60..1440 => format!("em {} h {:02}", minutes / 60, minutes % 60),
        _ => {
            const DAYS: [&str; 7] = ["dom", "seg", "ter", "qua", "qui", "sex", "sáb"];
            let day = DAYS[reset.weekday().num_days_from_sunday() as usize];
            format!("{day} {}", reset.format("%H:%M"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local(s: &str) -> chrono::DateTime<chrono::FixedOffset> {
        chrono::DateTime::parse_from_rfc3339(s).unwrap()
    }

    #[test]
    fn under_an_hour_shows_minutes() {
        assert_eq!(renew_text(local("2026-11-09T10:42:30-03:00"), at("2026-11-09T13:00:00Z")), "em 42 min");
    }

    #[test]
    fn under_a_day_shows_hours_and_minutes() {
        assert_eq!(renew_text(local("2026-11-09T12:10:00-03:00"), at("2026-11-09T13:00:00Z")), "em 2 h 10");
        assert_eq!(renew_text(local("2026-11-09T12:00:00-03:00"), at("2026-11-09T13:00:00Z")), "em 2 h");
        assert_eq!(renew_text(local("2026-11-09T13:04:00-03:00"), at("2026-11-09T13:00:00Z")), "em 3 h 04");
    }

    #[test]
    fn a_day_or_more_shows_weekday_and_time() {
        // 11/11/2026 é quarta-feira.
        assert_eq!(renew_text(local("2026-11-11T06:59:00-03:00"), at("2026-11-09T13:00:00Z")), "qua 06:59");
    }

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
