// Transparent always-on-top overlay with click-through outside the rail and a hover card.
mod antigravity;
mod claude;
mod codex;
mod quota;
mod win32;

use eframe::egui::{self, Color32, FontFamily, FontId, Pos2, Rect, Stroke, Vec2};
use std::f32::consts::TAU;

const TITLE: &str = "brunarias-rings-of-ai";
const WIDTH: f32 = CARD + 10.0 + RAIL;
const HEIGHT: f32 = 540.0;
const RAIL: f32 = 48.0;
const CARD: f32 = 290.0;
const ICON: f32 = 15.0;
const GAP: f32 = 6.0; // between agents
const SLOT: f32 = 57.0;
const RESTING: f32 = 0.9;
const ALERT: f32 = 0.85;
const OPENING: f32 = 0.3;

// Agents in rail order, with their installed app icons.
const AGENTS: [(&str, &[u8]); 3] = [
    ("Claude", include_bytes!("../assets/claude.png")),
    ("Codex", include_bytes!("../assets/codex.png")),
    ("Antigravity", include_bytes!("../assets/antigravity.png")),
];
const CODEX: usize = 1;
const ANTIGRAVITY: usize = 2;

type Source = Result<Vec<quota::Window>, String>;

// Last reading in agent order. A failed source does not hide the others.
type Reading = std::sync::Arc<std::sync::Mutex<Vec<Source>>>;

fn read_all() -> Vec<Source> {
    vec![claude::read_windows(), codex::read_windows(), antigravity::read_windows()]
}

fn keep_reading(reading: Reading, ctx: egui::Context) {
    std::thread::spawn(move || loop {
        let mut fresh = read_all();
        // Keep the last Antigravity reading when it closes and mark it stale after reset.
        if fresh[ANTIGRAVITY].is_err() {
            if let Some(Ok(last)) = reading.lock().unwrap().get(ANTIGRAVITY) {
                fresh[ANTIGRAVITY] = Ok(last.clone());
            }
        }
        *reading.lock().unwrap() = fresh;
        ctx.request_repaint();
        std::thread::sleep(std::time::Duration::from_secs(2));
        win32::release_idle_pages();
        std::thread::sleep(std::time::Duration::from_secs(178));
    });
}

struct App {
    reading: Reading,
    icons: Vec<egui::TextureHandle>,
    placed: bool,
    passthrough: bool,
}

fn usage_color(used: f32) -> Color32 {
    if used >= ALERT {
        Color32::from_rgb(255, 80, 70)
    } else if used >= 0.6 {
        Color32::from_rgb(255, 191, 35)
    } else {
        Color32::from_rgb(0, 218, 146)
    }
}

const GRAY: Color32 = Color32::from_rgb(154, 154, 154);

fn ring(painter: &egui::Painter, center: Pos2, w: &quota::Window, stale: bool) {
    let (radius, width) = (12.5, 2.5);
    painter.circle_stroke(center, radius, Stroke::new(width, Color32::from_gray(42)));
    if stale {
        return;
    }
    let used = w.used.clamp(0.0, 1.0);
    if used > 0.0 {
        let color = usage_color(w.used);
        let n = ((64.0 * used).ceil() as usize).max(1);
        let points: Vec<Pos2> = (0..=n).map(|i| center + Vec2::angled(-TAU / 4.0 + TAU * used * i as f32 / n as f32) * radius).collect();
        // Draw rounded ends separately from the egui stroke.
        painter.circle_filled(points[0], width / 2.0, color);
        painter.circle_filled(points[n], width / 2.0, color);
        painter.add(egui::Shape::line(points, Stroke::new(width, color)));
    }
}

// "Claude · 7 days Opus" → "7 days Opus".
fn window_name(w: &quota::Window) -> &str {
    w.label.split_once(" · ").map_or(&w.label, |(_, name)| name)
}

// Brighten the rail when a visible window reaches the alert threshold.
fn alert(reading: &[Source], now: chrono::DateTime<chrono::Utc>) -> bool {
    reading.iter().flatten().any(|ws| rail_window(ws, now).is_some_and(|w| !w.stale(now) && w.used >= ALERT))
}

// Short labels include the group or model initial when applicable, as in G5h or O7d.
fn short_name(w: &quota::Window) -> String {
    let name = window_name(w);
    let (group, window) = match name.find("5 h").or_else(|| name.find("7 days")) {
        Some(at) => (name[..at].trim(), &name[at..]),
        None => ("", name),
    };
    // Claude puts the model after the window duration.
    let (group, window) = match window.strip_prefix("7 days") {
        Some(model) if group.is_empty() => (model.trim(), "7d".to_string()),
        Some(_) => (group, "7d".to_string()),
        None => (group, window.replace(" min", "m").replace(' ', "")),
    };
    let initial = match group {
        "" => String::new(),
        "Claude and GPT" => "C".into(),
        g => g.split_whitespace().last().and_then(|word| word.chars().next()).map(String::from).unwrap_or_default(),
    };
    initial + &window
}

// The rail includes only Gemini from Antigravity. The card shows all windows.
fn on_rail(windows: &[quota::Window]) -> Vec<quota::Window> {
    let gemini = ["Antigravity · Gemini 5 h", "Antigravity · Gemini 7 days"];
    windows.iter().filter(|w| !w.label.starts_with("Antigravity") || gemini.contains(&w.label.as_str())).cloned().collect()
}

fn rail_window(windows: &[quota::Window], now: chrono::DateTime<chrono::Utc>) -> Option<quota::Window> {
    let windows = on_rail(windows);
    let weekly = |w: &quota::Window| window_name(w).contains("7 days");
    windows.iter()
        .filter(|w| weekly(w) && w.used > 0.9 && !w.stale(now))
        .max_by(|a, b| a.used.total_cmp(&b.used))
        .or_else(|| windows.iter().find(|w| window_name(w).ends_with("5 h")))
        .or_else(|| windows.iter().find(|w| !weekly(w)))
        .cloned()
}

fn renew_line(w: &quota::Window, now: chrono::DateTime<chrono::Utc>) -> String {
    w.resets_at
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|t| format!("resets {}", quota::renew_text(t.with_timezone(&chrono::Local).fixed_offset(), now)))
        .unwrap_or_default()
}

fn hovered_agent(count: usize, offset: f32) -> Option<(usize, f32)> {
    if !(0.0..rail_height(count)).contains(&offset) {
        return None;
    }
    let agent = (offset / (SLOT + GAP)) as usize;
    Some((agent, agent as f32 * (SLOT + GAP) + SLOT / 2.0))
}

fn rail_height(count: usize) -> f32 {
    SLOT * count as f32 + GAP * count.saturating_sub(1) as f32
}

fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}

const TEXT: Color32 = Color32::from_gray(238);
const SOFT: Color32 = Color32::from_rgb(141, 141, 144);

impl App {
    fn icon(&self, painter: &egui::Painter, center: Pos2, agent: usize, size: f32) {
        let rect = Rect::from_center_size(center, Vec2::splat(size));
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        painter.image(self.icons[agent].id(), rect, uv, Color32::WHITE);
    }

    fn draw_rail(&self, painter: &egui::Painter, rail: Rect, reading: &[Source], now: chrono::DateTime<chrono::Utc>) {
        const STEPS: u32 = 32;
        let mut background = egui::Mesh::default();
        for (edge, direction) in [(rail.top(), -1.0), (rail.bottom(), 1.0)] {
            for step in 0..=STEPS {
                let t = if direction < 0.0 { STEPS - step } else { step } as f32 / STEPS as f32;
                let rest = 1.0 - t;
                let x = rail.left() + rail.width() * t * t * (3.0 - 2.0 * t);
                let y = edge + direction * (87.0 * rest * rest * t + 45.0 * rest * t * t + 38.0 * t * t * t);
                let v = background.vertices.len() as u32;
                background.colored_vertex(Pos2::new(x - 0.5, y), Color32::TRANSPARENT);
                background.colored_vertex(Pos2::new((x + 0.5).min(rail.right()), y), Color32::BLACK);
                background.colored_vertex(Pos2::new(rail.right(), y), Color32::BLACK);
                if v >= 3 {
                    background.add_triangle(v - 3, v - 2, v);
                    background.add_triangle(v - 2, v + 1, v);
                    background.add_triangle(v - 2, v - 1, v + 1);
                    background.add_triangle(v - 1, v + 2, v + 1);
                }
            }
        }
        painter.add(egui::Shape::mesh(background));
        let x = rail.center().x;
        for (i, source) in reading.iter().enumerate() {
            let y = rail.min.y + i as f32 * (SLOT + GAP);
            let center = Pos2::new(x, y + 18.0);
            let window = source.as_ref().ok().and_then(|windows| rail_window(windows, now));
            let usage = if let Some(w) = window {
                let stale = w.stale(now);
                ring(painter, center, &w, stale);
                painter.text(Pos2::new(x, y + 45.0), egui::Align2::CENTER_TOP, short_name(&w), FontId::proportional(9.0), GRAY);
                if stale { "?".to_owned() } else { format!("{:.0}%", w.used * 100.0) }
            } else {
                painter.circle_stroke(center, 12.5, Stroke::new(2.5, Color32::from_gray(42)));
                "!".to_owned()
            };
            self.icon(painter, center, i, ICON);
            painter.text(Pos2::new(x, y + 32.0), egui::Align2::CENTER_TOP, usage, semibold(11.0), TEXT);
        }
    }

    fn draw_card(&self, painter: &egui::Painter, anchor: Pos2, bounds: Rect, agent: usize, source: &Source, now: chrono::DateTime<chrono::Utc>) {
        const ROW: f32 = 51.0;
        const HEAD: f32 = 32.0;
        const PAD: f32 = 16.0;
        let error = source.as_ref().err().map(|e| painter.layout(e.clone(), FontId::proportional(12.0), GRAY, CARD - 2.0 * PAD));
        let body = source.as_ref().map_or_else(|_| error.as_ref().map_or(0.0, |g| g.size().y), |ws| ws.len() as f32 * ROW);
        let height = (2.0 * PAD + HEAD + body).min(bounds.height());
        let top = (anchor.y - height / 2.0).clamp(bounds.min.y, bounds.max.y - height);
        let card = Rect::from_min_size(Pos2::new(anchor.x - 10.0 - CARD, top), Vec2::new(CARD, height));
        let fill = Color32::from_gray(8);
        painter.rect_filled(card, 18.0, fill);
        let pointer_y = anchor.y.clamp(card.min.y + 26.0, card.max.y - 26.0);
        painter.add(egui::Shape::convex_polygon(
            vec![Pos2::new(card.max.x - 1.0, pointer_y - 8.0), Pos2::new(anchor.x - 2.0, pointer_y), Pos2::new(card.max.x - 1.0, pointer_y + 8.0)],
            fill,
            Stroke::NONE,
        ));

        let painter = painter.with_clip_rect(card);
        let left = card.min.x + PAD;
        let right = card.max.x - PAD;
        let header = card.min.y + PAD + ICON / 2.0;
        self.icon(&painter, Pos2::new(left + ICON / 2.0, header), agent, ICON);
        painter.text(Pos2::new(left + ICON + 8.0, header), egui::Align2::LEFT_CENTER, format!("{} Usage", AGENTS[agent].0), semibold(15.0), TEXT);

        let mut y = card.min.y + PAD + HEAD;
        match source {
            Ok(windows) => {
                for w in windows {
                    let stale = w.stale(now);
                    painter.text(Pos2::new(left, y), egui::Align2::LEFT_TOP, window_name(w), FontId::proportional(12.0), TEXT);
                    let bar = Rect::from_min_max(Pos2::new(left, y + 18.0), Pos2::new(right, y + 23.0));
                    painter.rect_filled(bar, 2.5, Color32::from_gray(42));
                    if !stale && w.used > 0.0 {
                        let fill = Rect::from_min_max(bar.min, Pos2::new(bar.min.x + bar.width() * w.used.min(1.0), bar.max.y));
                        painter.rect_filled(fill, 2.5, usage_color(w.used));
                    }
                    let usage = if stale { "—".to_owned() } else { format!("{:.0}% Used", w.used * 100.0) };
                    painter.text(Pos2::new(left, y + 29.0), egui::Align2::LEFT_TOP, usage, semibold(11.0), TEXT);
                    let reset = match (stale, w.resets_at.is_some()) {
                        (true, _) => format!("Open {} to refresh", AGENTS[agent].0),
                        (false, true) => renew_line(w, now),
                        (false, false) => "No usage since reset".into(),
                    };
                    painter.text(Pos2::new(right, y + 29.0), egui::Align2::RIGHT_TOP, reset, FontId::proportional(11.0), SOFT);
                    y += ROW;
                }
            }
            Err(_) => {
                if let Some(error) = error {
                    painter.galley(Pos2::new(left, y), error, GRAY);
                }
            }
        }
    }
}

impl eframe::App for App {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = &ui.ctx().clone();
        let ppp = ctx.pixels_per_point();
        win32::strip_frame();
        if !self.placed {
            if let Some(size) = ctx.input(|i| i.viewport().monitor_size) {
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(Pos2::new(
                    size.x - WIDTH,
                    size.y / 2.0 - HEIGHT / 2.0,
                )));
                self.placed = true;
            }
        }

        let now = chrono::Utc::now();
        let mut reading = self.reading.lock().unwrap().clone();
        if let Some(Ok(windows)) = reading.get_mut(CODEX) {
            *windows = codex::after_reset(windows, now);
        }
        let alert = alert(&reading, now);

        let height = rail_height(reading.len());
        let rect = ui.max_rect();
        let offset = (rect.height() - height) / 2.0;

        // Click-through prevents hover events. Read the cursor and window position from Windows.
        let (rail_offset, near) = match (win32::cursor(), win32::window_rect()) {
            (Some((x, y)), Some((right, top))) => {
                let (x, y, right, top) = (x as f32 / ppp, y as f32 / ppp, right as f32 / ppp, top as f32 / ppp);
                let top = top + offset;
                let rail_left = right - RAIL;
                let on = x >= rail_left && x < right;
                let near = x >= rail_left - 120.0 && y >= top - 60.0 && y <= top + height + 60.0;
                (on.then_some(y - top), near)
            }
            _ => (None, false),
        };
        // Brighten the rail when the pointer is nearby or a visible window is in alert.
        let open = ctx.animate_bool_with_time_and_easing(egui::Id::unique("open"), near || alert, OPENING, egui::emath::easing::cubic_in_out);

        let hovered = rail_offset.and_then(|offset| hovered_agent(reading.len(), offset));
        let hovering_rail = hovered.is_some();
        if hovering_rail == self.passthrough && win32::set_passthrough(!hovering_rail) {
            self.passthrough = !hovering_rail;
        }

        let opacity = egui::lerp(RESTING..=1.0, open);

        let rail = Rect::from_min_size(Pos2::new(rect.max.x - RAIL, rect.min.y + offset), Vec2::new(RAIL, height));
        let mut painter = ui.painter().clone();
        painter.set_opacity(opacity);
        self.draw_rail(&painter, rail, &reading, now);
        if let Some((agent, center)) = hovered {
            let anchor = Pos2::new(rail.min.x, rail.min.y + center);
            self.draw_card(ui.painter(), anchor, rect, agent, &reading[agent], now);
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

// Use the Windows Segoe UI font in place of egui's default.
fn use_windows_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let dir = std::path::Path::new("C:/Windows/Fonts");
    for (name, file, family) in [
        ("segoe", "segoeui.ttf", FontFamily::Proportional),
        ("segoe-sb", "seguisb.ttf", FontFamily::Name("semibold".into())),
    ] {
        let list = fonts.families.entry(family).or_default();
        if let Ok(bytes) = std::fs::read(dir.join(file)) {
            fonts.font_data.insert(name.into(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
            list.insert(0, name.into());
        }
    }
    // Fall back to the proportional font when Segoe UI Semibold is unavailable.
    let proportional = fonts.families[&FontFamily::Proportional].clone();
    fonts.families.get_mut(&FontFamily::Name("semibold".into())).unwrap().extend(proportional);
    ctx.set_fonts(fonts);
}

fn load_icons(ctx: &egui::Context) -> Vec<egui::TextureHandle> {
    AGENTS
        .iter()
        .map(|(name, png)| {
            let img = image::load_from_memory(png).expect("embedded icon").to_rgba8();
            let size = [img.width() as usize, img.height() as usize];
            let color = egui::ColorImage::from_rgba_unmultiplied(size, &img);
            ctx.load_texture(*name, color, egui::TextureOptions::LINEAR)
        })
        .collect()
}

fn main() -> eframe::Result {
    // --read prints one quota reading without opening the overlay.
    if std::env::args().any(|a| matches!(a.as_str(), "--read" | "--ler")) {
        let now = chrono::Utc::now();
        for source in read_all() {
            match source {
                Ok(windows) => {
                    for w in windows {
                        let stale = if w.stale(now) { " (reset, stale reading)" } else { "" };
                        let reset = w.resets_at.as_deref().unwrap_or("?");
                        println!("{}: {:.0}% used, resets {reset}{stale}", w.label, w.used * 100.0);
                    }
                }
                Err(e) => eprintln!("{e}"),
            }
        }
        return Ok(());
    }
    if !win32::first_instance() {
        return Ok(());
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(TITLE)
            .with_inner_size([WIDTH, HEIGHT])
            .with_transparent(true)
            .with_decorations(false)
            .with_always_on_top()
            .with_taskbar(false)
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        TITLE,
        options,
        Box::new(|cc| {
            use_windows_fonts(&cc.egui_ctx);
            let reading: Reading = Default::default();
            keep_reading(reading.clone(), cc.egui_ctx.clone());
            Ok(Box::new(App {
                reading,
                icons: load_icons(&cc.egui_ctx),
                placed: false,
                passthrough: false,
            }))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(label: &str) -> quota::Window {
        quota::Window { label: label.into(), used: 0.1, resets_at: None }
    }

    #[test]
    fn weekly_window_replaces_hours_only_above_ninety_percent_and_before_reset() {
        let now = "2026-11-09T17:00:00Z".parse().unwrap();
        let hours = window("Claude · 5 h");
        let mut weekly = quota::Window { used: 0.9, ..window("Claude · 7 days") };
        assert_eq!(rail_window(&[weekly.clone(), hours.clone()], now), Some(hours.clone()));
        weekly.used = 0.91;
        assert_eq!(rail_window(&[hours.clone(), weekly.clone()], now), Some(weekly.clone()));
        weekly.resets_at = Some("2026-11-09T16:00:00Z".into());
        assert_eq!(rail_window(&[hours.clone(), weekly], now), Some(hours));
    }

    #[test]
    fn hover_selects_the_agent_at_the_current_rail_height() {
        let (first, center) = hovered_agent(2, 12.0).unwrap();
        assert_eq!(first, 0);
        let (second, _) = hovered_agent(2, 2.0 * center + GAP + 2.0).unwrap();
        assert_eq!(second, 1);
        assert_eq!(hovered_agent(2, rail_height(2) + 1.0), None);
        assert_eq!(hovered_agent(2, -1.0), None);
    }

    #[test]
    fn antigravity_rail_keeps_only_gemini() {
        let windows = [
            window("Antigravity · Gemini 5 h"),
            window("Antigravity · Gemini 7 days"),
            window("Antigravity · Claude and GPT 5 h"),
            window("Antigravity · Gemini 4 Argon 7 days"),
            window("Claude · 7 days Opus"),
        ];
        let labels: Vec<String> = on_rail(&windows).into_iter().map(|w| w.label).collect();
        assert_eq!(labels, ["Antigravity · Gemini 5 h", "Antigravity · Gemini 7 days", "Claude · 7 days Opus"]);
    }

    #[test]
    fn opus_week_has_its_own_label() {
        assert_eq!(short_name(&window("Claude · 7 days")), "7d");
        assert_eq!(short_name(&window("Claude · 7 days Opus")), "O7d");
        assert_eq!(short_name(&window("Antigravity · Gemini 7 days")), "G7d");
    }

    #[test]
    fn only_windows_on_the_rail_raise_the_alert() {
        let now = chrono::Utc::now();
        let high = |label: &str| quota::Window { label: label.into(), used: 0.9, resets_at: None };
        let argon: Vec<Source> = vec![Ok(vec![window("Antigravity · Gemini 5 h"), high("Antigravity · Gemini 4 Argon 7 days")])];
        assert!(!alert(&argon, now));
        let gemini: Vec<Source> = vec![Ok(vec![high("Antigravity · Gemini 5 h")])];
        assert!(alert(&gemini, now));
    }

    #[test]
    fn rail_height_has_one_slot_per_provider() {
        assert_eq!(rail_height(0), 0.0);
        assert_eq!(rail_height(2), 2.0 * SLOT + GAP);
    }
}
