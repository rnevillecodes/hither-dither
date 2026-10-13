//! Editor windows on Windows: a Win32 window owned by the main window, so it stays above it
//! and minimises with it, holding the plugin's view as a child (`kPlatformTypeHWND`).
//!
//! View sizes are in physical pixels. The window's DPI is passed to the plugin through
//! `IPlugViewContentScaleSupport`, at opening and on `WM_DPICHANGED`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Once;

use tauri::{Runtime, WebviewWindow};
use vst3::Steinberg::{
    FIDString, IPlugFrame, IPlugView, IPlugViewContentScaleSupport,
    IPlugViewContentScaleSupportTrait, IPlugViewTrait, ViewRect, kPlatformTypeHWND, kResultOk,
    kResultTrue,
};
use vst3::{ComPtr, ComRef, ComWrapper};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{BLACK_BRUSH, GetStockObject, HBRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForWindow};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

use super::{EditorWindow, detach, rect, view_size};
use crate::plugins::host::PlugFrame;

pub const PLATFORM_TYPE: FIDString = kPlatformTypeHWND;

/// The main window's handle.
pub type Parent = HWND;

pub fn parent_of<R: Runtime>(window: &WebviewWindow<R>) -> Option<Parent> {
    window.hwnd().ok().map(|h| HWND(h.0 as _))
}

/// Native editors can always be shown on Windows.
pub fn available() -> Result<(), String> {
    Ok(())
}

const CLASS_NAME: PCWSTR = w!("HitherDitherPluginEditor");
const EX_STYLE: WINDOW_EX_STYLE = WINDOW_EX_STYLE(0);

fn style(resizable: bool) -> WINDOW_STYLE {
    let fixed = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN;
    if resizable {
        fixed | WS_THICKFRAME | WS_MAXIMIZEBOX
    } else {
        fixed
    }
}

/// What the window procedure needs, kept in the window's user data.
struct State {
    /// The attached view; taken when it is detached.
    view: RefCell<Option<ComPtr<IPlugView>>>,
    on_close: Box<dyn Fn()>,
    /// The client size last given to the view with `onSize`.
    size: Cell<(i32, i32)>,
    resizable: bool,
    /// Set once `WM_DESTROY` arrives, however the window is being destroyed.
    destroying: Cell<bool>,
}

pub struct Window {
    hwnd: HWND,
    state: Rc<State>,
    _frame: ComWrapper<PlugFrame>,
}

impl Window {
    /// Creates a window for `view`, sized to it, attaches the view and shows the window.
    /// `on_close` runs when the user closes the window or Windows destroys it with its owner.
    pub fn open(
        parent: Option<Parent>,
        title: &str,
        view: ComPtr<IPlugView>,
        on_close: Box<dyn Fn()>,
    ) -> Result<Window, String> {
        register_class();
        let resizable = unsafe { view.canResize() } == kResultTrue;
        let title: Vec<u16> = title.encode_utf16().chain([0]).collect();
        let instance = unsafe { GetModuleHandleW(None) }
            .ok()
            .map(|m| HINSTANCE(m.0));
        let hwnd = unsafe {
            CreateWindowExW(
                EX_STYLE,
                CLASS_NAME,
                PCWSTR(title.as_ptr()),
                style(resizable),
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                400,
                300,
                parent,
                None,
                instance,
                None,
            )
        }
        .map_err(|e| e.message())?;
        let state = Rc::new(State {
            view: RefCell::new(None),
            on_close,
            size: Cell::new((0, 0)),
            resizable,
            destroying: Cell::new(false),
        });
        // Freed at WM_NCDESTROY.
        let user_data = Box::into_raw(Box::new(state.clone()));
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, user_data as isize) };

        let raw = hwnd.0 as isize;
        let frame = ComWrapper::new(PlugFrame::new(Box::new(move |view, rect| unsafe {
            resize_view(HWND(raw as _), view, rect)
        })));
        let frame_ptr = frame
            .as_com_ref::<IPlugFrame>()
            .map_or(std::ptr::null_mut(), |r| r.as_ptr());
        unsafe { view.setFrame(frame_ptr) };
        set_scale(&view, unsafe { GetDpiForWindow(hwnd) });
        set_client_size(hwnd, view_size(&view).unwrap_or((400, 300)));
        state.size.set(client_size(hwnd));
        place_over(hwnd, parent);

        if unsafe { view.attached(hwnd.0, PLATFORM_TYPE) } != kResultOk {
            unsafe {
                view.setFrame(std::ptr::null_mut());
                let _ = DestroyWindow(hwnd);
            }
            return Err("the plugin didn't accept the window".into());
        }
        *state.view.borrow_mut() = Some(view.clone());
        // Some plugins settle on their size only once attached.
        if let Some(size) = view_size(&view) {
            set_client_size(hwnd, size);
        }
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOW);
        }
        Ok(Window {
            hwnd,
            state,
            _frame: frame,
        })
    }
}

impl EditorWindow for Window {
    fn front(&self) {
        unsafe {
            let cmd = if IsIconic(self.hwnd).as_bool() {
                SW_RESTORE
            } else {
                SW_SHOW
            };
            let _ = ShowWindow(self.hwnd, cmd);
            let _ = SetForegroundWindow(self.hwnd);
        }
    }

    fn set_visible(&self, visible: bool) {
        // Hiding keeps the window's place, so it comes back where it was.
        let cmd = if visible { SW_SHOWNA } else { SW_HIDE };
        unsafe {
            let _ = ShowWindow(self.hwnd, cmd);
        }
    }

    fn close(self) {
        let view = self.state.view.borrow_mut().take();
        if let Some(view) = view {
            detach(&view);
        }
        if !self.state.destroying.get() {
            unsafe {
                let _ = DestroyWindow(self.hwnd);
            }
        }
    }
}

fn register_class() {
    static REGISTER: Once = Once::new();
    REGISTER.call_once(|| unsafe {
        let instance = GetModuleHandleW(None).map(|m| HINSTANCE(m.0)).ok();
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            hInstance: instance.unwrap_or_default(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: HBRUSH(GetStockObject(BLACK_BRUSH).0),
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        RegisterClassExW(&class);
    });
}

fn state_of(hwnd: HWND) -> Option<Rc<State>> {
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const Rc<State>;
    (!ptr.is_null()).then(|| unsafe { (*ptr).clone() })
}

fn current_view(state: &State) -> Option<ComPtr<IPlugView>> {
    state.view.borrow().clone()
}

fn set_scale(view: &ComPtr<IPlugView>, dpi: u32) {
    if let Some(scale) = view.cast::<IPlugViewContentScaleSupport>() {
        unsafe { scale.setContentScaleFactor(dpi as f32 / 96.0) };
    }
}

/// The width and height the window's frame adds to its client area at `dpi`.
fn frame_size(hwnd: HWND, dpi: u32) -> (i32, i32) {
    let style = WINDOW_STYLE(unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) } as u32);
    let mut r = RECT::default();
    let _ = unsafe { AdjustWindowRectExForDpi(&mut r, style, false, EX_STYLE, dpi) };
    (r.right - r.left, r.bottom - r.top)
}

fn client_size(hwnd: HWND) -> (i32, i32) {
    let mut r = RECT::default();
    let _ = unsafe { GetClientRect(hwnd, &mut r) };
    (r.right - r.left, r.bottom - r.top)
}

fn set_client_size(hwnd: HWND, (width, height): (i32, i32)) {
    let (fw, fh) = frame_size(hwnd, unsafe { GetDpiForWindow(hwnd) });
    let flags = SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE;
    let _ = unsafe { SetWindowPos(hwnd, None, 0, 0, width + fw, height + fh, flags) };
}

/// Puts a new window over the middle of the main window.
fn place_over(hwnd: HWND, parent: Option<HWND>) {
    let (mut own, mut main) = (RECT::default(), RECT::default());
    let Some(parent) = parent else { return };
    if unsafe { GetWindowRect(hwnd, &mut own).and(GetWindowRect(parent, &mut main)) }.is_err() {
        return;
    }
    let x = main.left + ((main.right - main.left) - (own.right - own.left)) / 2;
    let y = main.top + ((main.bottom - main.top) - (own.bottom - own.top)) / 3;
    let flags = SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE;
    let _ = unsafe { SetWindowPos(hwnd, None, x.max(main.left), y.max(main.top), 0, 0, flags) };
}

/// `IPlugFrame::resizeView`: resizes the window to the view's new size and tells the view.
unsafe fn resize_view(hwnd: HWND, view: *mut IPlugView, new_size: ViewRect) -> bool {
    let Some(state) = state_of(hwnd) else {
        return false;
    };
    let size = (
        new_size.right - new_size.left,
        new_size.bottom - new_size.top,
    );
    if size.0 <= 0 || size.1 <= 0 {
        return false;
    }
    let before = state.size.get();
    set_client_size(hwnd, size);
    // WM_SIZE calls onSize when the client size changed; otherwise call it here.
    if state.size.get() == before
        && let Some(view) = unsafe { ComRef::from_raw(view) }
    {
        state.size.set(size);
        let mut r = rect(size.0, size.1);
        unsafe { view.onSize(&mut r) };
    }
    true
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let Some(state) = state_of(hwnd) else {
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    };
    match msg {
        WM_SIZE if wparam.0 as u32 != SIZE_MINIMIZED => {
            let size = (
                (lparam.0 & 0xFFFF) as i32,
                ((lparam.0 >> 16) & 0xFFFF) as i32,
            );
            if size != state.size.get()
                && let Some(view) = current_view(&state)
            {
                state.size.set(size);
                let mut r = rect(size.0, size.1);
                unsafe { view.onSize(&mut r) };
            }
            LRESULT(0)
        }
        WM_SIZING if state.resizable => {
            if let Some(view) = current_view(&state) {
                unsafe { constrain(hwnd, &view, wparam.0 as u32, lparam.0 as *mut RECT) };
            }
            LRESULT(1)
        }
        WM_DPICHANGED => {
            let dpi = (wparam.0 & 0xFFFF) as u32;
            let suggested = unsafe { *(lparam.0 as *const RECT) };
            let (mut width, mut height) = (
                suggested.right - suggested.left,
                suggested.bottom - suggested.top,
            );
            if let Some(view) = current_view(&state) {
                set_scale(&view, dpi);
                if let Some((w, h)) = view_size(&view) {
                    let (fw, fh) = frame_size(hwnd, dpi);
                    (width, height) = (w + fw, h + fh);
                }
            }
            let flags = SWP_NOZORDER | SWP_NOACTIVATE;
            let _ = unsafe {
                SetWindowPos(
                    hwnd,
                    None,
                    suggested.left,
                    suggested.top,
                    width,
                    height,
                    flags,
                )
            };
            LRESULT(0)
        }
        WM_CLOSE => {
            // The manager detaches the view and destroys the window.
            (state.on_close)();
            LRESULT(0)
        }
        WM_DESTROY => {
            state.destroying.set(true);
            // Destroyed along with the main window rather than by the manager: detach the
            // view while its window still exists, then let the manager forget it.
            let view = state.view.borrow_mut().take();
            if let Some(view) = view {
                detach(&view);
                drop(view);
                (state.on_close)();
            }
            LRESULT(0)
        }
        WM_NCDESTROY => {
            let ptr = unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) } as *mut Rc<State>;
            if !ptr.is_null() {
                drop(unsafe { Box::from_raw(ptr) });
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// `WM_SIZING`: fits the window being dragged to a size the view accepts, keeping the edge
/// opposite the one dragged in place.
unsafe fn constrain(hwnd: HWND, view: &ComPtr<IPlugView>, edge: u32, window: *mut RECT) {
    let Some(window) = (unsafe { window.as_mut() }) else {
        return;
    };
    let (fw, fh) = frame_size(hwnd, unsafe { GetDpiForWindow(hwnd) });
    let mut r = rect(
        window.right - window.left - fw,
        window.bottom - window.top - fh,
    );
    if unsafe { view.checkSizeConstraint(&mut r) } != kResultTrue {
        return;
    }
    let (width, height) = (r.right - r.left + fw, r.bottom - r.top + fh);
    if matches!(edge, WMSZ_LEFT | WMSZ_TOPLEFT | WMSZ_BOTTOMLEFT) {
        window.left = window.right - width;
    } else {
        window.right = window.left + width;
    }
    if matches!(edge, WMSZ_TOP | WMSZ_TOPLEFT | WMSZ_TOPRIGHT) {
        window.top = window.bottom - height;
    } else {
        window.bottom = window.top + height;
    }
}
