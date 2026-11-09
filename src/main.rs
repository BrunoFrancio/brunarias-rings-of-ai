// Janela transparente presa na borda direita, sempre por cima,
// que deixa o clique passar fora dos anéis e mostra um cartão no hover.
mod antigravity;
mod claude;
mod codex;
mod quota;

use eframe::egui::{self, Color32, FontFamily, FontId, Pos2, Rect, Stroke, Vec2};
use std::f32::consts::TAU;

const TITLE: &str = "aneis-de-cota";
const WIDTH: f32 = 340.0; // cartão de 290 + folga de 8 + trilho de 38 + margem
const HEIGHT: f32 = 530.0; // cabe tudo aberto: 3 janelas do Claude, 2 do Codex e 5 do Antigravity
const RAIL: f32 = 38.0;
const MARGIN: f32 = 4.0; // entre o trilho e a borda da tela
const CARD: f32 = 290.0;
const PAD: f32 = 6.0;
const ICON: f32 = 16.0;
const HEADER: f32 = ICON + 5.0;
const SLOT: f32 = 42.0; // anel de 28 + rótulo + folga
const SLOT_RESTING: f32 = 32.0; // sem rótulo
const SEP: f32 = 7.0;
const RESTING: f32 = 0.45; // opacidade com o mouse longe
const ALERT: f32 = 0.85;

// Agentes na ordem do trilho. O ícone é o do app instalado de cada um.
const AGENTS: [(&str, &[u8]); 3] = [
    ("Claude", include_bytes!("../assets/claude.png")),
    ("Codex", include_bytes!("../assets/codex.png")),
    ("Antigravity", include_bytes!("../assets/antigravity.png")),
];
const ANTIGRAVITY: usize = 2;

type Source = Result<Vec<quota::Window>, String>;

// Última leitura de cada agente, na ordem de AGENTS.
// Uma fonte com erro não esconde a outra.
type Reading = std::sync::Arc<std::sync::Mutex<Vec<Source>>>;

fn read_all() -> Vec<Source> {
    vec![claude::read_windows(), codex::read_windows(), antigravity::read_windows()]
}

fn keep_reading(reading: Reading, ctx: egui::Context) {
    std::thread::spawn(move || loop {
        let mut fresh = read_all();
        // O Antigravity só responde aberto. Fechado, vale a última leitura,
        // e a regra da leitura vencida cuida dela quando a janela renovar.
        if fresh[ANTIGRAVITY].is_err() {
            if let Some(Ok(last)) = reading.lock().unwrap().get(ANTIGRAVITY) {
                fresh[ANTIGRAVITY] = Ok(last.clone());
            }
        }
        *reading.lock().unwrap() = fresh;
        ctx.request_repaint();
        std::thread::sleep(std::time::Duration::from_secs(2));
        release_idle_pages();
        std::thread::sleep(std::time::Duration::from_secs(178));
    });
}

// O driver de vídeo e a primeira conexão tocam ~50 MB que não voltam a ser usados.
// O Windows só os devolve quando falta RAM, e até lá o Gerenciador de Tarefas os conta.
// Medido: de 85 MB para cerca de 8 MB, que é o que o app usa de fato.
fn release_idle_pages() {
    use windows_sys::Win32::System::{ProcessStatus::K32EmptyWorkingSet, Threading::GetCurrentProcess};
    unsafe { K32EmptyWorkingSet(GetCurrentProcess()) };
}

struct App {
    reading: Reading,
    icons: Vec<egui::TextureHandle>,
    placed: bool,
    passthrough: bool,
}

fn cursor() -> Option<(i32, i32)> {
    use windows_sys::Win32::{Foundation::POINT, UI::WindowsAndMessaging::GetCursorPos};
    let mut p = POINT { x: 0, y: 0 };
    (unsafe { GetCursorPos(&mut p) } != 0).then_some((p.x, p.y))
}

fn own_window() -> Option<windows_sys::Win32::Foundation::HWND> {
    let title: Vec<u16> = TITLE.encode_utf16().chain(Some(0)).collect();
    let hwnd = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW(std::ptr::null(), title.as_ptr()) };
    (!hwnd.is_null()).then_some(hwnd)
}

// Borda direita e topo da janela, em pixels da tela.
fn window_rect() -> Option<(i32, i32)> {
    let hwnd = own_window()?;
    let mut r = unsafe { std::mem::zeroed::<windows_sys::Win32::Foundation::RECT>() };
    (unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, &mut r) } != 0).then_some((r.right, r.top))
}

// Deixa o clique atravessar a janela, ou não. São os mesmos bits que o winit liga,
// mas o comando dele reaplica todos os estilos e a moldura pisca por um quadro.
fn set_passthrough(on: bool) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let Some(hwnd) = own_window() else { return false };
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let bits = WS_EX_TRANSPARENT | WS_EX_LAYERED;
        let new = if on { style | bits } else { style & !bits };
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new as isize);
    }
    true
}

// O Windows 11 desenha borda, canto arredondado e sombra até em janela sem moldura.
// Roda a cada quadro e só age quando a moldura voltou, porque o winit a devolve
// nos primeiros quadros, depois da primeira limpeza.
fn strip_frame() {
    use windows_sys::Win32::Graphics::Dwm::{DwmExtendFrameIntoClientArea, DwmSetWindowAttribute};
    use windows_sys::Win32::UI::Controls::MARGINS;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let Some(hwnd) = own_window() else { return };
    let frame = WS_CAPTION | WS_THICKFRAME | WS_SYSMENU;
    if unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) } as u32 & frame == 0 {
        return;
    }
    let set = |attr: u32, value: u32| unsafe {
        DwmSetWindowAttribute(hwnd, attr, &value as *const u32 as _, 4);
    };
    set(2, 1); // DWMWA_NCRENDERING_POLICY = DISABLED: sem sombra
    set(33, 1); // DWMWA_WINDOW_CORNER_PREFERENCE = DONOTROUND
    set(34, 0xFFFF_FFFE); // DWMWA_BORDER_COLOR = NONE
    let margins = MARGINS { cxLeftWidth: 0, cxRightWidth: 0, cyTopHeight: 0, cyBottomHeight: 0 };
    unsafe {
        DwmExtendFrameIntoClientArea(hwnd, &margins);
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let clean = (style & !frame) | WS_POPUP;
        SetWindowLongPtrW(hwnd, GWL_STYLE, clean as isize);
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
        // O winit guarda 1 px no topo para a sombra, e ele aparece como linha.
        // A região da janela começa abaixo dele.
        let mut r = std::mem::zeroed();
        GetWindowRect(hwnd, &mut r);
        let rgn = windows_sys::Win32::Graphics::Gdi::CreateRectRgn(0, 1, r.right - r.left, r.bottom - r.top);
        windows_sys::Win32::Graphics::Gdi::SetWindowRgn(hwnd, rgn, 1);
    }
}

fn usage_color(used: f32) -> Color32 {
    if used >= ALERT {
        Color32::from_rgb(226, 75, 74)
    } else if used >= 0.6 {
        Color32::from_rgb(239, 159, 39)
    } else {
        Color32::from_rgb(93, 202, 165)
    }
}

const GRAY: Color32 = Color32::from_rgb(154, 154, 154);

fn ring(painter: &egui::Painter, center: Pos2, w: &quota::Window, stale: bool) {
    let radius = 11.0;
    painter.circle_stroke(center, radius, Stroke::new(3.0, Color32::from_white_alpha(36)));
    // Janela que já renovou: o uso de agora é desconhecido, então nada de arco.
    if stale {
        painter.text(center, egui::Align2::CENTER_CENTER, "?", FontId::proportional(11.0), GRAY);
        return;
    }
    let steps = 48;
    let points: Vec<Pos2> = (0..=((steps as f32 * w.used.min(1.0)) as usize))
        .map(|i| center + Vec2::angled(-TAU / 4.0 + TAU * i as f32 / steps as f32) * radius)
        .collect();
    painter.add(egui::Shape::line(points, Stroke::new(3.0, usage_color(w.used))));
    let pct = format!("{:.0}", w.used * 100.0);
    painter.text(center, egui::Align2::CENTER_CENTER, pct, FontId::proportional(9.5), Color32::from_gray(238));
}

// "Claude · 7 dias Opus" → "7 dias Opus".
fn window_name(w: &quota::Window) -> &str {
    w.label.split_once(" · ").map_or(&w.label, |(_, name)| name)
}

// Rótulo curto embaixo do anel: "5h", "7d", "60m". No Antigravity vem com a
// inicial do grupo: "G5h" (Gemini), "C7d" (Claude e GPT), "A7d" (Argon).
fn short_name(w: &quota::Window) -> String {
    let name = window_name(w);
    let (group, window) = match name.find("5 h").or_else(|| name.find("7 dias")) {
        Some(at) => (name[..at].trim(), &name[at..]),
        None => ("", name),
    };
    let window = if window.starts_with("7 dias") { "7d".into() } else { window.replace(" min", "m").replace(' ', "") };
    let initial = match group {
        "" => String::new(),
        "Claude e GPT" => "C".into(),
        g => g.split_whitespace().last().and_then(|word| word.chars().next()).map(String::from).unwrap_or_default(),
    };
    initial + &window
}

// Em repouso, só a janela de 5 horas de cada agente.
// Se um agente não tiver janela de 5 horas, fica a primeira que houver.
fn visible(windows: &[quota::Window], expanded: bool) -> Vec<&quota::Window> {
    if expanded {
        return windows.iter().collect();
    }
    let five: Vec<&quota::Window> = windows.iter().filter(|w| window_name(w).ends_with("5 h")).collect();
    if five.is_empty() { windows.iter().take(1).collect() } else { five }
}

fn renew_line(w: &quota::Window, now: chrono::DateTime<chrono::Utc>) -> String {
    w.resets_at
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|t| format!("renova {}", quota::renew_text(t.with_timezone(&chrono::Local).fixed_offset(), now)))
        .unwrap_or_default()
}

fn slot(expanded: bool) -> f32 {
    if expanded { SLOT } else { SLOT_RESTING }
}

fn group_height(source: &Source, expanded: bool) -> f32 {
    HEADER + source.as_ref().map_or(slot(expanded), |ws| visible(ws, expanded).len() as f32 * slot(expanded))
}

fn rail_height(reading: &[Source], expanded: bool) -> f32 {
    let groups: f32 = reading.iter().map(|s| group_height(s, expanded)).sum();
    2.0 * PAD + groups + SEP * reading.len().saturating_sub(1) as f32
}

fn title_font() -> FontId {
    FontId::new(13.0, FontFamily::Name("semibold".into()))
}

impl App {
    fn icon(&self, painter: &egui::Painter, center: Pos2, agent: usize, size: f32) {
        let rect = Rect::from_center_size(center, Vec2::splat(size));
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        painter.image(self.icons[agent].id(), rect, uv, Color32::WHITE);
    }

    fn draw_rail(&self, painter: &egui::Painter, rail: Rect, reading: &[Source], now: chrono::DateTime<chrono::Utc>, lit: f32, expanded: bool) {
        let bg = egui::lerp(56.0..=217.0, lit) as u8;
        painter.rect_filled(rail, 12.0, Color32::from_rgba_unmultiplied(18, 18, 20, bg));
        let x = rail.center().x;
        let mut y = rail.min.y + PAD;
        for (i, source) in reading.iter().enumerate() {
            if i > 0 {
                let mid = y + SEP / 2.0;
                painter.line_segment([Pos2::new(x - 10.0, mid), Pos2::new(x + 10.0, mid)], Stroke::new(1.0, Color32::from_white_alpha(30)));
                y += SEP;
            }
            self.icon(painter, Pos2::new(x, y + ICON / 2.0), i, ICON);
            y += HEADER;
            match source {
                Ok(windows) => {
                    for w in visible(windows, expanded) {
                        ring(painter, Pos2::new(x, y + 14.0), w, w.stale(now));
                        if expanded {
                            painter.text(Pos2::new(x, y + 28.5), egui::Align2::CENTER_TOP, short_name(w), FontId::monospace(9.0), Color32::from_gray(168));
                        }
                        y += slot(expanded);
                    }
                }
                Err(_) => {
                    painter.text(Pos2::new(x, y + 14.0), egui::Align2::CENTER_CENTER, "!", FontId::proportional(13.0), GRAY);
                    y += slot(expanded);
                }
            }
        }
    }

    fn draw_card(&self, painter: &egui::Painter, top_right: Pos2, reading: &[Source], now: chrono::DateTime<chrono::Utc>) {
        const ROW: f32 = 34.0;
        const LABEL: f32 = 124.0;
        let inner = CARD - 28.0;
        let errors: Vec<Option<std::sync::Arc<egui::Galley>>> = reading
            .iter()
            .map(|s| s.as_ref().err().map(|e| painter.layout(e.clone(), FontId::proportional(12.0), GRAY, inner)))
            .collect();
        let body: f32 = reading
            .iter()
            .zip(&errors)
            .map(|(s, e)| 22.0 + s.as_ref().map_or_else(|_| e.as_ref().map_or(0.0, |g| g.size().y + 8.0), |ws| ws.len() as f32 * ROW))
            .sum();
        let height = 20.0 + body + 10.0 * reading.len().saturating_sub(1) as f32;
        let card = Rect::from_min_size(Pos2::new(top_right.x - CARD, top_right.y), Vec2::new(CARD, height));
        painter.rect(card, 12.0, Color32::from_rgb(29, 30, 33), Stroke::new(0.5, Color32::from_white_alpha(26)), egui::StrokeKind::Inside);

        let left = card.min.x + 14.0;
        let right = card.max.x - 14.0;
        let mut y = card.min.y + 10.0;
        for (i, source) in reading.iter().enumerate() {
            if i > 0 {
                painter.line_segment([Pos2::new(left, y + 2.0), Pos2::new(right, y + 2.0)], Stroke::new(1.0, Color32::from_white_alpha(20)));
                y += 10.0;
            }
            self.icon(painter, Pos2::new(left + 8.0, y + 8.0), i, 16.0);
            painter.text(Pos2::new(left + 22.0, y + 8.0), egui::Align2::LEFT_CENTER, AGENTS[i].0, title_font(), Color32::from_gray(238));
            y += 22.0;
            match source {
                Ok(windows) => {
                    for w in windows {
                        let mid = y + 8.0;
                        painter.text(Pos2::new(left, mid), egui::Align2::LEFT_CENTER, window_name(w), FontId::proportional(12.0), Color32::from_gray(238));
                        let sub_pos = Pos2::new(left + LABEL, y + 15.0);
                        if w.stale(now) {
                            painter.text(Pos2::new(left + LABEL, mid), egui::Align2::LEFT_CENTER, "já renovou", FontId::proportional(12.0), GRAY);
                            let hint = format!("abra o {} para atualizar", AGENTS[i].0);
                            painter.text(sub_pos, egui::Align2::LEFT_TOP, hint, FontId::proportional(11.0), GRAY);
                        } else {
                            let bar = Rect::from_min_max(Pos2::new(left + LABEL, mid - 2.5), Pos2::new(right - 36.0, mid + 2.5));
                            painter.rect_filled(bar, 3.0, Color32::from_white_alpha(26));
                            let fill = Rect::from_min_max(bar.min, Pos2::new(bar.min.x + bar.width() * w.used.clamp(0.0, 1.0), bar.max.y));
                            painter.rect_filled(fill, 3.0, usage_color(w.used));
                            let pct = format!("{:.0}%", w.used * 100.0);
                            painter.text(Pos2::new(right, mid), egui::Align2::RIGHT_CENTER, pct, FontId::proportional(12.0), Color32::from_gray(238));
                            painter.text(sub_pos, egui::Align2::LEFT_TOP, renew_line(w, now), FontId::proportional(11.0), GRAY);
                        }
                        y += ROW;
                    }
                }
                Err(_) => {
                    if let Some(g) = &errors[i] {
                        let h = g.size().y;
                        painter.galley(Pos2::new(left, y), g.clone(), GRAY);
                        y += h + 8.0;
                    }
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
        strip_frame();
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
        let reading = self.reading.lock().unwrap().clone();
        let alert = reading.iter().flatten().flatten().any(|w| !w.stale(now) && w.used >= ALERT);
        // A altura do teste de proximidade é a do trilho aberto, para ele não fechar
        // enquanto o mouse desce pelos anéis que acabaram de aparecer.
        let full_height = rail_height(&reading, true);

        // Com o clique passando, a janela não recebe hover. O cursor e a posição da
        // janela vêm do Windows: a do egui fica velha logo depois de abrir.
        let (hovering_rail, near) = match (cursor(), window_rect()) {
            (Some((x, y)), Some((right, top))) => {
                let (x, y, right, top) = (x as f32 / ppp, y as f32 / ppp, right as f32 / ppp, top as f32 / ppp);
                let rail_left = right - MARGIN - RAIL;
                let on = x >= rail_left && x <= right && y >= top && y <= top + full_height;
                let near = x >= rail_left - 120.0 && y >= top - 60.0 && y <= top + full_height + 60.0;
                (on, near)
            }
            _ => (false, false),
        };
        if hovering_rail == self.passthrough && set_passthrough(!hovering_rail) {
            self.passthrough = !hovering_rail;
        }

        // Discreto em repouso, só com a janela de 5 horas; acende e abre tudo
        // com o mouse perto ou com alguma cota alta.
        let expanded = near || alert;
        let height = rail_height(&reading, expanded);
        let target = if expanded { 1.0 } else { RESTING };
        let opacity = ctx.animate_value_with_time(egui::Id::unique("opacidade"), target, 0.25);
        let lit = (opacity - RESTING) / (1.0 - RESTING);

        let rect = ui.max_rect();
        let rail = Rect::from_min_size(Pos2::new(rect.max.x - MARGIN - RAIL, rect.min.y), Vec2::new(RAIL, height));
        let mut painter = ui.painter().clone();
        painter.set_opacity(opacity);
        if reading.is_empty() {
            painter.rect_filled(rail, 12.0, Color32::from_black_alpha(56));
        } else {
            self.draw_rail(&painter, rail, &reading, now, lit, expanded);
        }
        if hovering_rail && !reading.is_empty() {
            self.draw_card(ui.painter(), Pos2::new(rail.min.x - 8.0, rail.min.y), &reading, now);
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

// Segoe UI, a fonte do Windows, no lugar da fonte padrão do egui.
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
    // A família de título cai na proporcional se a Segoe Semibold faltar.
    let proportional = fonts.families[&FontFamily::Proportional].clone();
    fonts.families.get_mut(&FontFamily::Name("semibold".into())).unwrap().extend(proportional);
    ctx.set_fonts(fonts);
}

fn load_icons(ctx: &egui::Context) -> Vec<egui::TextureHandle> {
    AGENTS
        .iter()
        .map(|(name, png)| {
            let img = image::load_from_memory(png).expect("ícone embutido").to_rgba8();
            let size = [img.width() as usize, img.height() as usize];
            let color = egui::ColorImage::from_rgba_unmultiplied(size, &img);
            ctx.load_texture(*name, color, egui::TextureOptions::LINEAR)
        })
        .collect()
}

fn main() -> eframe::Result {
    // `aneis-de-cota --ler`: lê as cotas uma vez e mostra no terminal, sem abrir a janela.
    if std::env::args().any(|a| a == "--ler") {
        let now = chrono::Utc::now();
        for source in read_all() {
            match source {
                Ok(windows) => {
                    for w in windows {
                        let stale = if w.stale(now) { " (já renovou, valor antigo)" } else { "" };
                        let reset = w.resets_at.as_deref().unwrap_or("?");
                        println!("{}: {:.0}% usado, renova {reset}{stale}", w.label, w.used * 100.0);
                    }
                }
                Err(e) => eprintln!("{e}"),
            }
        }
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
