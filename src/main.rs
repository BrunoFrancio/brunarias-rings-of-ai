// Prova: janela transparente presa na borda direita, sempre por cima,
// que deixa o clique passar fora dos anéis e mostra um cartão no hover.
use eframe::egui::{self, Color32, Pos2, Stroke, Vec2};
use std::f32::consts::TAU;

const TITLE: &str = "aneis-de-cota";
const WIDTH: f32 = 320.0;
const HEIGHT: f32 = 200.0;
const RAIL: f32 = 56.0;

struct Ring {
    label: &'static str,
    used: f32,
}

struct App {
    rings: Vec<Ring>,
    placed: bool,
    stripped: bool,
    passthrough: bool,
}

fn cursor() -> Option<(i32, i32)> {
    use windows_sys::Win32::{Foundation::POINT, UI::WindowsAndMessaging::GetCursorPos};
    let mut p = POINT { x: 0, y: 0 };
    (unsafe { GetCursorPos(&mut p) } != 0).then_some((p.x, p.y))
}

// O Windows 11 desenha borda, canto arredondado e sombra até em janela sem moldura.
fn strip_frame() -> bool {
    use windows_sys::Win32::Graphics::Dwm::{DwmExtendFrameIntoClientArea, DwmSetWindowAttribute};
    use windows_sys::Win32::UI::Controls::MARGINS;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let title: Vec<u16> = TITLE.encode_utf16().chain(Some(0)).collect();
    let hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
    if hwnd.is_null() {
        return false;
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
        let clean = (style & !(WS_CAPTION | WS_THICKFRAME | WS_SYSMENU)) | WS_POPUP;
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
    true
}

fn ring(painter: &egui::Painter, center: Pos2, radius: f32, used: f32) {
    painter.circle_stroke(center, radius, Stroke::new(5.0, Color32::from_white_alpha(40)));
    let color = if used < 0.6 {
        Color32::from_rgb(80, 200, 120)
    } else if used < 0.85 {
        Color32::from_rgb(240, 180, 60)
    } else {
        Color32::from_rgb(230, 80, 70)
    };
    let steps = 64;
    let points: Vec<Pos2> = (0..=((steps as f32 * used) as usize))
        .map(|i| center + Vec2::angled(-TAU / 4.0 + TAU * i as f32 / steps as f32) * radius)
        .collect();
    painter.add(egui::Shape::line(points, Stroke::new(5.0, color)));
}

impl eframe::App for App {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = &ui.ctx().clone();
        let ppp = ctx.pixels_per_point();
        if !self.stripped {
            self.stripped = strip_frame();
        }
        if !self.placed {
            if let Some(size) = ctx.input(|i| i.viewport().monitor_size) {
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(Pos2::new(
                    size.x - WIDTH,
                    size.y / 2.0 - HEIGHT / 2.0,
                )));
                self.placed = true;
            }
        }

        // Com o clique passando, a janela não recebe hover. O cursor vem do Windows.
        let outer = ctx.input(|i| i.viewport().outer_rect);
        let hovering_rail = match (cursor(), outer) {
            (Some((x, y)), Some(r)) => {
                let (x, y) = (x as f32 / ppp, y as f32 / ppp);
                x >= r.max.x - RAIL && x <= r.max.x && y >= r.min.y && y <= r.max.y
            }
            _ => false,
        };
        if hovering_rail == self.passthrough {
            self.passthrough = !hovering_rail;
            ctx.send_viewport_cmd(egui::ViewportCommand::MousePassthrough(self.passthrough));
            // O winit recalcula os estilos a cada troca e devolve a moldura.
            self.stripped = false;
        }

        let rect = ui.max_rect();
        let painter = ui.painter();
        let rail = egui::Rect::from_min_max(Pos2::new(rect.max.x - RAIL, rect.min.y), rect.max);
        painter.rect_filled(rail, 14.0, Color32::from_black_alpha(170));
        for (i, r) in self.rings.iter().enumerate() {
            let c = Pos2::new(rail.center().x, rail.min.y + 34.0 + i as f32 * 58.0);
            ring(painter, c, 18.0, r.used);
        }
        if hovering_rail {
            let card = egui::Rect::from_min_size(
                Pos2::new(rail.min.x - 236.0, rail.min.y),
                Vec2::new(228.0, 40.0 + self.rings.len() as f32 * 44.0),
            );
            painter.rect_filled(card, 12.0, Color32::from_black_alpha(210));
            for (i, r) in self.rings.iter().enumerate() {
                painter.text(
                    card.min + Vec2::new(14.0, 18.0 + i as f32 * 44.0),
                    egui::Align2::LEFT_TOP,
                    format!("{}  {:.0}% usado", r.label, r.used * 100.0),
                    egui::FontId::proportional(15.0),
                    Color32::WHITE,
                );
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

fn main() -> eframe::Result {
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
        Box::new(|_| {
            Ok(Box::new(App {
                rings: vec![
                    Ring { label: "Claude · 5 h", used: 0.42 },
                    Ring { label: "Codex · 7 dias", used: 0.09 },
                    Ring { label: "Claude · 7 dias", used: 0.88 },
                ],
                placed: false,
                stripped: false,
                passthrough: false,
            }))
        }),
    )
}
