//! Editor windows on macOS: an `NSWindow` added as a child window of the main window, so it
//! stays above it and moves with it, with the plugin's view attached to its content view
//! (`kPlatformTypeNSView`). View sizes are in points; plugins follow the backing scale
//! themselves.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use objc2::rc::{Retained, Weak};
use objc2::runtime::ProtocolObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSBackingStoreType, NSWindow, NSWindowDelegate, NSWindowOrderingMode, NSWindowStyleMask,
};
use objc2_foundation::{
    NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};
use tauri::{Runtime, WebviewWindow};
use vst3::Steinberg::{
    FIDString, IPlugFrame, IPlugView, IPlugViewTrait, ViewRect, kPlatformTypeNSView, kResultOk,
    kResultTrue,
};
use vst3::{ComPtr, ComRef, ComWrapper};

use super::{EditorWindow, detach, rect, view_size};
use crate::plugins::host::PlugFrame;

pub const PLATFORM_TYPE: FIDString = kPlatformTypeNSView;

/// The main window's `NSWindow`.
pub type Parent = *mut c_void;

pub fn parent_of<R: Runtime>(window: &WebviewWindow<R>) -> Option<Parent> {
    window.ns_window().ok().filter(|w| !w.is_null())
}

/// Native editors can always be shown on macOS.
pub fn available() -> Result<(), String> {
    Ok(())
}

/// What the window delegate needs.
struct State {
    window: RefCell<Weak<NSWindow>>,
    view: RefCell<Option<ComPtr<IPlugView>>>,
    on_close: Box<dyn Fn()>,
    /// The content size last given to the view with `onSize`.
    size: Cell<(i32, i32)>,
}

impl State {
    fn view(&self) -> Option<ComPtr<IPlugView>> {
        self.view.borrow().clone()
    }

    fn window(&self) -> Option<Retained<NSWindow>> {
        self.window.borrow().load()
    }
}

struct Ivars {
    state: Rc<State>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "HitherDitherEditorDelegate"]
    #[ivars = Ivars]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl NSWindowDelegate for Delegate {
        #[unsafe(method(windowShouldClose:))]
        fn window_should_close(&self, _sender: &NSWindow) -> bool {
            // The manager detaches the view and closes the window.
            (self.ivars().state.on_close)();
            false
        }

        #[unsafe(method(windowWillResize:toSize:))]
        fn window_will_resize(&self, sender: &NSWindow, frame_size: NSSize) -> NSSize {
            let Some(view) = self.ivars().state.view() else {
                return frame_size;
            };
            let frame = NSRect::new(NSPoint::new(0.0, 0.0), frame_size);
            let content = sender.contentRectForFrameRect(frame).size;
            let mut r = rect(content.width.round() as i32, content.height.round() as i32);
            if unsafe { view.checkSizeConstraint(&mut r) } != kResultTrue {
                return frame_size;
            }
            let size = NSSize::new((r.right - r.left) as f64, (r.bottom - r.top) as f64);
            let content = NSRect::new(NSPoint::new(0.0, 0.0), size);
            sender.frameRectForContentRect(content).size
        }

        #[unsafe(method(windowDidResize:))]
        fn window_did_resize(&self, _notification: &NSNotification) {
            let state = &self.ivars().state;
            if let Some(window) = state.window() {
                notify_size(state, content_size(&window));
            }
        }
    }
);

impl Delegate {
    fn new(mtm: MainThreadMarker, state: Rc<State>) -> Retained<Delegate> {
        let this = Self::alloc(mtm).set_ivars(Ivars { state });
        unsafe { msg_send![super(this), init] }
    }
}

fn content_size(window: &NSWindow) -> (i32, i32) {
    let size = window.contentRectForFrameRect(window.frame()).size;
    (size.width.round() as i32, size.height.round() as i32)
}

/// Tells the view its new size, if it changed.
fn notify_size(state: &State, size: (i32, i32)) {
    if size == state.size.get() {
        return;
    }
    if let Some(view) = state.view() {
        state.size.set(size);
        let mut r = rect(size.0, size.1);
        unsafe { view.onSize(&mut r) };
    }
}

pub struct Window {
    window: Retained<NSWindow>,
    parent: Option<Retained<NSWindow>>,
    state: Rc<State>,
    _delegate: Retained<Delegate>,
    _frame: ComWrapper<PlugFrame>,
    /// Cleared on close, so a late `resizeView` doesn't touch a released window.
    alive: Arc<AtomicBool>,
}

impl Window {
    /// Creates a window for `view`, sized to it, attaches the view and shows the window.
    /// `on_close` runs when the user closes the window.
    pub fn open(
        parent: Option<Parent>,
        title: &str,
        view: ComPtr<IPlugView>,
        on_close: Box<dyn Fn()>,
    ) -> Result<Window, String> {
        let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
        let resizable = unsafe { view.canResize() } == kResultTrue;
        let (width, height) = view_size(&view).unwrap_or((400, 300));
        let mut style = NSWindowStyleMask::Titled | NSWindowStyleMask::Closable;
        if resizable {
            style |= NSWindowStyleMask::Resizable;
        }
        let content = NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(width as f64, height as f64),
        );
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                content,
                style,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        unsafe { window.setReleasedWhenClosed(false) };
        window.setTitle(&NSString::from_str(title));

        let state = Rc::new(State {
            window: RefCell::new(Weak::from_retained(&window)),
            view: RefCell::new(None),
            on_close,
            size: Cell::new((width, height)),
        });
        let delegate = Delegate::new(mtm, state.clone());
        window.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));

        let alive = Arc::new(AtomicBool::new(true));
        let flag = alive.clone();
        let window_ptr = Retained::as_ptr(&window) as usize;
        let delegate_ptr = Retained::as_ptr(&delegate) as usize;
        let frame = ComWrapper::new(PlugFrame::new(Box::new(move |view, rect| {
            // Plugins call this on the main thread while the window is open; the window and
            // delegate live until `close` clears the flag.
            flag.load(Ordering::Relaxed)
                && unsafe {
                    let window = &*(window_ptr as *const NSWindow);
                    let delegate = &*(delegate_ptr as *const Delegate);
                    resize_view(window, &delegate.ivars().state, view, rect)
                }
        })));
        let frame_ptr = frame
            .as_com_ref::<IPlugFrame>()
            .map_or(std::ptr::null_mut(), |r| r.as_ptr());
        unsafe { view.setFrame(frame_ptr) };

        let parent = parent.and_then(|p| unsafe { Retained::retain(p as *mut NSWindow) });
        place_over(&window, parent.as_deref());
        let Some(content_view) = window.contentView() else {
            unsafe { view.setFrame(std::ptr::null_mut()) };
            return Err("the window has no content view".into());
        };
        let attached = unsafe {
            view.attached(
                Retained::as_ptr(&content_view) as *mut c_void,
                PLATFORM_TYPE,
            )
        };
        if attached != kResultOk {
            unsafe { view.setFrame(std::ptr::null_mut()) };
            window.close();
            return Err("the plugin didn't accept the window".into());
        }
        *state.view.borrow_mut() = Some(view.clone());
        if let Some((w, h)) = view_size(&view) {
            window.setContentSize(NSSize::new(w as f64, h as f64));
        }
        if let Some(parent) = &parent {
            unsafe { parent.addChildWindow_ordered(&window, NSWindowOrderingMode::Above) };
        }
        window.makeKeyAndOrderFront(None);
        Ok(Window {
            window,
            parent,
            state,
            _delegate: delegate,
            _frame: frame,
            alive,
        })
    }
}

impl EditorWindow for Window {
    fn front(&self) {
        if self.window.isMiniaturized() {
            self.window.deminiaturize(None);
        }
        self.set_visible(true);
        self.window.makeKeyAndOrderFront(None);
    }

    fn set_visible(&self, visible: bool) {
        // Ordering out keeps the window's frame, so it comes back where it was. A child
        // window is detached first: ordering out an attached child misbehaves.
        if let Some(parent) = &self.parent {
            if visible {
                unsafe { parent.addChildWindow_ordered(&self.window, NSWindowOrderingMode::Above) };
            } else {
                parent.removeChildWindow(&self.window);
            }
        }
        if visible {
            self.window.orderFront(None);
        } else {
            self.window.orderOut(None);
        }
    }

    fn close(self) {
        self.alive.store(false, Ordering::Relaxed);
        let view = self.state.view.borrow_mut().take();
        if let Some(view) = view {
            detach(&view);
        }
        if let Some(parent) = &self.parent {
            parent.removeChildWindow(&self.window);
        }
        self.window.setDelegate(None);
        self.window.close();
    }
}

/// Puts a new window over the middle of the main window.
fn place_over(window: &NSWindow, parent: Option<&NSWindow>) {
    let Some(parent) = parent else {
        window.center();
        return;
    };
    let (main, own) = (parent.frame(), window.frame());
    let x = main.origin.x + (main.size.width - own.size.width) / 2.0;
    // AppKit's y axis points up: leave a third of the space above.
    let top = main.origin.y + main.size.height - (main.size.height - own.size.height) / 3.0;
    window.setFrameTopLeftPoint(NSPoint::new(x, top));
}

/// `IPlugFrame::resizeView`: resizes the window to the view's new size and tells the view.
unsafe fn resize_view(
    window: &NSWindow,
    state: &State,
    view: *mut IPlugView,
    new_size: ViewRect,
) -> bool {
    let size = (
        new_size.right - new_size.left,
        new_size.bottom - new_size.top,
    );
    if size.0 <= 0 || size.1 <= 0 {
        return false;
    }
    let before = state.size.get();
    window.setContentSize(NSSize::new(size.0 as f64, size.1 as f64));
    // windowDidResize calls onSize when the size changed; otherwise call it here.
    if state.size.get() == before
        && let Some(view) = unsafe { ComRef::from_raw(view) }
    {
        state.size.set(size);
        let mut r = rect(size.0, size.1);
        unsafe { view.onSize(&mut r) };
    }
    true
}
