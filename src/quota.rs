// A quota window reported by an agent.

#[derive(Debug, Clone, PartialEq)]
pub struct Window {
    pub label: String,
    pub used: f32, // 0.0 to 1.0
    pub resets_at: Option<String>, // RFC 3339
}

impl Window {
    /// A reset after the last reading makes current usage unknown.
    pub fn stale(&self, now: chrono::DateTime<chrono::Utc>) -> bool {
        self.resets_at
            .as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .is_some_and(|reset| reset <= now)
    }
}

/// Formats the reset time relative to now, or as a weekday and time in the reset offset.
pub fn renew_text(reset: chrono::DateTime<chrono::FixedOffset>, now: chrono::DateTime<chrono::Utc>) -> String {
    use chrono::Datelike;
    let minutes = (reset.with_timezone(&chrono::Utc) - now).num_minutes();
    match minutes {
        ..60 => format!("in {minutes} min"),
        60..1440 if minutes % 60 == 0 => format!("in {} h", minutes / 60),
        60..1440 => format!("in {} h {:02}", minutes / 60, minutes % 60),
        _ => {
            const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
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
        assert_eq!(renew_text(local("2026-11-09T10:42:30-03:00"), at("2026-11-09T13:00:00Z")), "in 42 min");
    }

    #[test]
    fn under_a_day_shows_hours_and_minutes() {
        assert_eq!(renew_text(local("2026-11-09T12:10:00-03:00"), at("2026-11-09T13:00:00Z")), "in 2 h 10");
        assert_eq!(renew_text(local("2026-11-09T12:00:00-03:00"), at("2026-11-09T13:00:00Z")), "in 2 h");
        assert_eq!(renew_text(local("2026-11-09T13:04:00-03:00"), at("2026-11-09T13:00:00Z")), "in 3 h 04");
    }

    #[test]
    fn a_day_or_more_shows_weekday_and_time() {
        // November 11, 2026 is a Wednesday.
        assert_eq!(renew_text(local("2026-11-11T06:59:00-03:00"), at("2026-11-09T13:00:00Z")), "Wed 06:59");
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
        assert!(!window(Some("yesterday")).stale(at("2026-11-07T05:00:00Z")));
    }
}
