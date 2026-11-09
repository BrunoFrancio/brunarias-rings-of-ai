// O que o app pede direto ao Windows: a própria janela, o cursor e a memória.

// Uma cópia só. Duas cópias achariam a mesma janela pelo título e uma mexeria na da outra.
// A trava fica com o processo até ele fechar.
pub fn first_instance() -> bool {
    use windows_sys::Win32::{Foundation::{GetLastError, ERROR_ALREADY_EXISTS}, System::Threading::CreateMutexW};
    let name: Vec<u16> = format!("Local\\{}", crate::TITLE).encode_utf16().chain(Some(0)).collect();
    unsafe {
        CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        GetLastError() != ERROR_ALREADY_EXISTS
    }
}

// O driver de vídeo e a primeira conexão tocam ~50 MB que não voltam a ser usados.
// O Windows só os devolve quando falta RAM, e até lá o Gerenciador de Tarefas os conta.
// Medido: de 85 MB para cerca de 8 MB, que é o que o app usa de fato.
pub fn release_idle_pages() {
    use windows_sys::Win32::System::{ProcessStatus::K32EmptyWorkingSet, Threading::GetCurrentProcess};
    unsafe { K32EmptyWorkingSet(GetCurrentProcess()) };
}

pub fn cursor() -> Option<(i32, i32)> {
    use windows_sys::Win32::{Foundation::POINT, UI::WindowsAndMessaging::GetCursorPos};
    let mut p = POINT { x: 0, y: 0 };
    (unsafe { GetCursorPos(&mut p) } != 0).then_some((p.x, p.y))
}

fn own_window() -> Option<windows_sys::Win32::Foundation::HWND> {
    let title: Vec<u16> = crate::TITLE.encode_utf16().chain(Some(0)).collect();
    let hwnd = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW(std::ptr::null(), title.as_ptr()) };
    (!hwnd.is_null()).then_some(hwnd)
}

// Borda direita e topo da janela, em pixels da tela.
pub fn window_rect() -> Option<(i32, i32)> {
    let hwnd = own_window()?;
    let mut r = unsafe { std::mem::zeroed::<windows_sys::Win32::Foundation::RECT>() };
    (unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, &mut r) } != 0).then_some((r.right, r.top))
}

// Deixa o clique atravessar a janela, ou não. São os mesmos bits que o winit liga,
// mas o comando dele reaplica todos os estilos e a moldura pisca por um quadro.
pub fn set_passthrough(on: bool) -> bool {
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
pub fn strip_frame() {
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
