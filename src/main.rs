// Transparent always-on-top overlay with click-through outside the rail and a hover card.
mod antigravity;
mod claude;
mod codex;
mod quota;
mod win32;

use eframe::egui::{self, Color32, FontFamily, FontId, Pos2, Rect, Stroke, Vec2};
use std::f32::consts::TAU;

const TITLE: &str = "brunarias-rings-of-ai";
const WIDTH: f32 = 356.0; // 290 px card + 10 px gap + 52 px rail + margin
const HEIGHT: f32 = 540.0;
const RAIL: f32 = 52.0;
const MARGIN: f32 = 4.0;
const CARD: f32 = 290.0;
const ICON: f32 = 18.0;
const HEADER: f32 = 6.0;
const CAP_TOP: f32 = 7.0;
const CAP_BOTTOM: f32 = 3.0;
const GAP: f32 = 6.0; // between agents
const SLOT: f32 = 64.0; // ring, percentage, and window label
const SLOT_RESTING: f32 = 56.0; // ring and percentage
const RESTING: f32 = 0.45;
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
    let (radius, width) = (15.0, 3.5);
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

// Keep the rail expanded when one of its visible windows reaches the alert threshold.
fn alert(reading: &[Source], now: chrono::DateTime<chrono::Utc>) -> bool {
    reading.iter().flatten().any(|ws| on_rail(ws).iter().any(|w| !w.stale(now) && w.used >= ALERT))
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

// Show the five-hour window at rest, falling back to the first available window.
fn shows_resting(windows: &[quota::Window]) -> Vec<bool> {
    let five = |w: &quota::Window| window_name(w).ends_with("5 h");
    let any_five = windows.iter().any(five);
    windows.iter().enumerate().map(|(i, w)| if any_five { five(w) } else { i == 0 }).collect()
}

fn renew_line(w: &quota::Window, now: chrono::DateTime<chrono::Utc>) -> String {
    w.resets_at
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|t| format!("resets {}", quota::renew_text(t.with_timezone(&chrono::Local).fixed_offset(), now)))
        .unwrap_or_default()
}

// An extra ring starts at zero height and pushes the following rings down as it expands.
fn slot(resting: bool, open: f32) -> f32 {
    if resting { egui::lerp(SLOT_RESTING..=SLOT, open) } else { SLOT * open }
}

fn group_height(source: &Source, open: f32) -> f32 {
    HEADER + source.as_ref().map_or(slot(true, open), |ws| shows_resting(&on_rail(ws)).into_iter().map(|r| slot(r, open)).sum())
}


fn agent_height(source: &Source, open: f32) -> f32 {
    CAP_TOP + group_height(source, open) + CAP_BOTTOM
}

fn hovered_agent(reading: &[Source], offset: f32, open: f32) -> Option<(usize, f32)> {
    let mut top = 0.0;
    for (index, source) in reading.iter().enumerate() {
        let height = agent_height(source, open);
        let bottom = top + height + if index + 1 < reading.len() { GAP } else { 0.0 };
        if (top..bottom).contains(&offset) {
            return Some((index, top + height / 2.0));
        }
        top = bottom;
    }
    None
}

fn rail_height(reading: &[Source], open: f32) -> f32 {
    let agents: f32 = reading.iter().map(|s| agent_height(s, open)).sum();
    agents + GAP * reading.len().saturating_sub(1) as f32
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

    fn draw_rail(&self, painter: &egui::Painter, rail: Rect, reading: &[Source], now: chrono::DateTime<chrono::Utc>, open: f32) {
        painter.rect_filled(rail, RAIL / 2.0, Color32::from_gray(8));
        let x = rail.center().x;
        let late = ((open - 0.5) / 0.5).clamp(0.0, 1.0);
        let painter = painter.with_clip_rect(rail);
        let mut top = rail.min.y;
        for (i, source) in reading.iter().enumerate() {
            let mut y = top + CAP_TOP + HEADER;
            match source {
                Ok(windows) => {
                    let windows = on_rail(windows);
                    for (w, resting) in windows.iter().zip(shows_resting(&windows)) {
                        let mut faded = painter.clone();
                        faded.set_opacity(painter.opacity() * late);
                        let visible = if resting { &painter } else { &faded };
                        let center = Pos2::new(x, y + 17.0);
                        let stale = w.stale(now);
                        ring(visible, center, w, stale);
                        if resting {
                            self.icon(visible, center, i, ICON);
                        }
                        let usage = if stale { "?".to_owned() } else { format!("{:.0}%", w.used * 100.0) };
                        visible.text(Pos2::new(x, y + 37.0), egui::Align2::CENTER_TOP, usage, semibold(12.0), TEXT);
                        faded.text(Pos2::new(x, y + 51.0), egui::Align2::CENTER_TOP, short_name(w), FontId::proportional(9.0), GRAY);
                        y += slot(resting, open);
                    }
                }
                Err(_) => {
                    let center = Pos2::new(x, y + 17.0);
                    painter.circle_stroke(center, 15.0, Stroke::new(3.5, Color32::from_gray(42)));
                    self.icon(&painter, center, i, ICON);
                    painter.text(Pos2::new(x, y + 37.0), egui::Align2::CENTER_TOP, "!", semibold(12.0), GRAY);
                }
            }
            top += agent_height(source, open) + GAP;
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
        // Use the expanded height for proximity so the rail stays open over newly revealed rings.
        let full_height = rail_height(&reading, 1.0);

        // Click-through prevents hover events. Read the cursor and window position from Windows.
        let (rail_offset, near) = match (win32::cursor(), win32::window_rect()) {
            (Some((x, y)), Some((right, top))) => {
                let (x, y, right, top) = (x as f32 / ppp, y as f32 / ppp, right as f32 / ppp, top as f32 / ppp);
                let rail_left = right - MARGIN - RAIL;
                let on = x >= rail_left && x <= right - MARGIN;
                let near = x >= rail_left - 120.0 && y >= top - 60.0 && y <= top + full_height + 60.0;
                (on.then_some(y - top), near)
            }
            _ => (None, false),
        };
        // Animate height, opacity, and extra rings together when proximity or usage expands the rail.
        let open = ctx.animate_bool_with_time_and_easing(egui::Id::unique("open"), near || alert, OPENING, egui::emath::easing::cubic_in_out);
        let height = rail_height(&reading, open);
        let hovered = rail_offset.and_then(|offset| hovered_agent(&reading, offset, open));
        let hovering_rail = hovered.is_some();
        if hovering_rail == self.passthrough && win32::set_passthrough(!hovering_rail) {
            self.passthrough = !hovering_rail;
        }

        let opacity = egui::lerp(RESTING..=1.0, open);

        let rect = ui.max_rect();
        let rail = Rect::from_min_size(Pos2::new(rect.max.x - MARGIN - RAIL, rect.min.y), Vec2::new(RAIL, height));
        let mut painter = ui.painter().clone();
        painter.set_opacity(opacity);
        if reading.is_empty() {
            painter.rect_filled(rail, RAIL / 2.0, Color32::from_black_alpha(56));
        } else {
            self.draw_rail(&painter, rail, &reading, now, open);
        }
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
    fn hover_selects_the_agent_at_the_current_rail_height() {
        let reading = vec![Ok(vec![window("Claude · 5 h"), window("Claude · 7 days")]), Err("closed".into())];
        for open in [0.0, 0.5, 1.0] {
            let (first, center) = hovered_agent(&reading, 12.0, open).unwrap();
            assert_eq!(first, 0);
            let (second, _) = hovered_agent(&reading, 2.0 * center + GAP + 2.0, open).unwrap();
            assert_eq!(second, 1);
            assert_eq!(hovered_agent(&reading, rail_height(&reading, open) + 1.0, open), None);
        }
        assert_eq!(hovered_agent(&reading, -1.0, 1.0), None);
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
    fn rail_grows_smoothly_between_resting_and_open() {
        let reading: Vec<Source> = vec![Ok(vec![window("Claude · 5 h"), window("Claude · 7 days")]), Err("closed".into())];
        let closed = rail_height(&reading, 0.0);
        let half = rail_height(&reading, 0.5);
        let open = rail_height(&reading, 1.0);
        let capsules = 2.0 * (CAP_TOP + HEADER + CAP_BOTTOM) + GAP;
        assert_eq!(closed, capsules + 2.0 * SLOT_RESTING);
        assert_eq!(open, capsules + 3.0 * SLOT);
        assert_eq!(half, (closed + open) / 2.0);
    }
}
