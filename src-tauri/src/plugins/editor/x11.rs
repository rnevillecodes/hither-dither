//! Editor windows on Linux: an X11 top-level window, attached with
//! `kPlatformTypeX11EmbedWindowID`. VST3 editors need X11, so this works on X11 sessions and
//! on Wayland through XWayland. When GTK itself runs on X11, the window is transient for the
//! main window, so it stays above it.
//!
//! There is no X event loop of our own: the X connection and the plugin's `IRunLoop` file
//! descriptors and timers are GLib sources on the main context, which Tauri's GTK loop runs.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::c_void;
use std::os::fd::AsRawFd;
use std::rc::{Rc, Weak};
use std::time::Duration;

use glib::{ControlFlow, IOCondition, MainContext, SourceId};
use gtk::prelude::*;
use tauri::{Runtime, WebviewWindow};
use vst3::Steinberg::Linux::{
    FileDescriptor, IEventHandler, IEventHandlerTrait, IRunLoop, IRunLoopTrait, ITimerHandler,
    ITimerHandlerTrait, TimerInterval,
};
use vst3::Steinberg::{
    FIDString, IPlugFrame, IPlugFrameTrait, IPlugView, IPlugViewTrait, ViewRect, kInvalidArgument,
    kPlatformTypeX11EmbedWindowID, kResultFalse, kResultOk, kResultTrue, tresult,
};
use vst3::{Class, ComPtr, ComRef, ComWrapper};
use x11rb::connection::Connection;
use x11rb::properties::WmSizeHints;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    AtomEnum, ClientMessageEvent, ConfigureWindowAux, ConnectionExt as _, CreateWindowAux,
    EventMask, PropMode, StackMode, Window as XWindow, WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use super::{EditorWindow, detach, rect, view_size};

pub const PLATFORM_TYPE: FIDString = kPlatformTypeX11EmbedWindowID;

/// The main window's X11 window ID, when GTK runs on X11.
pub type Parent = u32;

const NO_X11: &str = "Plugin editors need X11 or XWayland, which this session doesn't have, \
                      so the effect's controls are shown instead.";

pub fn parent_of<R: Runtime>(window: &WebviewWindow<R>) -> Option<Parent> {
    let gdk_window = window.gtk_window().ok()?.window()?;
    let x11 = gdk_window.downcast::<gdkx11::X11Window>().ok()?;
    u32::try_from(x11.xid()).ok()
}

/// Whether an X11 display can be reached for editor windows. UI thread.
pub fn available() -> Result<(), String> {
    Display::get().map(|_| ())
}

x11rb::atom_manager! {
    Atoms: AtomsCookie {
        WM_PROTOCOLS,
        WM_DELETE_WINDOW,
        _NET_WM_NAME,
        _NET_ACTIVE_WINDOW,
        UTF8_STRING,
    }
}

/// The app's X connection, opened on first use and kept on the UI thread.
struct Display {
    conn: RustConnection,
    root: XWindow,
    black: u32,
    atoms: Atoms,
    /// The editor windows, for routing events.
    windows: RefCell<HashMap<XWindow, Weak<State>>>,
}

thread_local! {
    static DISPLAY: RefCell<Option<Rc<Display>>> = const { RefCell::new(None) };
}

impl Display {
    fn get() -> Result<Rc<Display>, String> {
        if let Some(d) = DISPLAY.with_borrow(|d| d.clone()) {
            return Ok(d);
        }
        let display = Rc::new(Display::connect().ok_or(NO_X11)?);
        let fd = display.conn.stream().as_raw_fd();
        // The X connection is read whenever it has data, on the GTK main loop.
        glib::source::unix_fd_add_local(fd, IOCondition::IN, |_, _| {
            if let Some(d) = DISPLAY.with_borrow(|d| d.clone()) {
                d.pump();
            }
            ControlFlow::Continue
        });
        DISPLAY.with_borrow_mut(|d| *d = Some(display.clone()));
        Ok(display)
    }

    fn connect() -> Option<Display> {
        std::env::var_os("DISPLAY")?;
        let (conn, screen) = x11rb::connect(None).ok()?;
        let atoms = Atoms::new(&conn).ok()?.reply().ok()?;
        let root = conn.setup().roots.get(screen)?;
        let (root, black) = (root.root, root.black_pixel);
        Some(Display {
            conn,
            root,
            black,
            atoms,
            windows: RefCell::default(),
        })
    }

    /// Handles the events that have arrived.
    fn pump(&self) {
        let mut events = Vec::new();
        while let Ok(Some(event)) = self.conn.poll_for_event() {
            events.push(event);
        }
        for event in events {
            let (window, handle): (XWindow, fn(&Display, &State, &Event)) = match &event {
                Event::ConfigureNotify(e) => (e.window, Display::configured),
                Event::ClientMessage(e) => (e.window, Display::client_message),
                _ => continue,
            };
            let state = self.windows.borrow().get(&window).and_then(Weak::upgrade);
            if let Some(state) = state {
                handle(self, &state, &event);
            }
        }
    }

    /// The user resized the window: fit it to a size the view accepts, then tell the view.
    fn configured(&self, state: &State, event: &Event) {
        let Event::ConfigureNotify(e) = event else {
            return;
        };
        let size = (i32::from(e.width), i32::from(e.height));
        if size == state.size.get() {
            return;
        }
        let Some(view) = state.view() else { return };
        let mut r = rect(size.0, size.1);
        if state.resizable && unsafe { view.checkSizeConstraint(&mut r) } == kResultTrue {
            let fitted = (r.right - r.left, r.bottom - r.top);
            if fitted != size {
                self.resize(state.window, fitted);
                return;
            }
        }
        state.size.set(size);
        let mut r = rect(size.0, size.1);
        unsafe { view.onSize(&mut r) };
    }

    fn client_message(&self, state: &State, event: &Event) {
        if let Event::ClientMessage(e) = event
            && e.type_ == self.atoms.WM_PROTOCOLS
            && e.data.as_data32()[0] == self.atoms.WM_DELETE_WINDOW
        {
            // The manager detaches the view and destroys the window.
            (state.on_close)();
        }
    }

    fn create(
        &self,
        title: &str,
        (width, height): (i32, i32),
        resizable: bool,
        parent: Option<Parent>,
    ) -> Result<XWindow, x11rb::errors::ReplyOrIdError> {
        let c = &self.conn;
        let window = c.generate_id()?;
        let aux = CreateWindowAux::new()
            .background_pixel(self.black)
            .event_mask(EventMask::STRUCTURE_NOTIFY);
        c.create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            window,
            self.root,
            0,
            0,
            width.max(1) as u16,
            height.max(1) as u16,
            0,
            WindowClass::INPUT_OUTPUT,
            x11rb::COPY_FROM_PARENT,
            &aux,
        )?;
        let utf8 = self.atoms.UTF8_STRING;
        c.change_property8(
            PropMode::REPLACE,
            window,
            AtomEnum::WM_NAME,
            AtomEnum::STRING,
            title.as_bytes(),
        )?;
        c.change_property8(
            PropMode::REPLACE,
            window,
            self.atoms._NET_WM_NAME,
            utf8,
            title.as_bytes(),
        )?;
        let delete = [self.atoms.WM_DELETE_WINDOW];
        c.change_property32(
            PropMode::REPLACE,
            window,
            self.atoms.WM_PROTOCOLS,
            AtomEnum::ATOM,
            &delete,
        )?;
        if let Some(parent) = parent {
            let owner = [parent];
            c.change_property32(
                PropMode::REPLACE,
                window,
                AtomEnum::WM_TRANSIENT_FOR,
                AtomEnum::WINDOW,
                &owner,
            )?;
        }
        if !resizable {
            self.fix_size(window, (width, height))?;
        }
        Ok(window)
    }

    /// Tells the window manager the window can't be resized from `size`.
    fn fix_size(
        &self,
        window: XWindow,
        size: (i32, i32),
    ) -> Result<(), x11rb::errors::ConnectionError> {
        let mut hints = WmSizeHints::new();
        hints.min_size = Some(size);
        hints.max_size = Some(size);
        hints.set_normal_hints(&self.conn, window)?;
        Ok(())
    }

    fn resize(&self, window: XWindow, (width, height): (i32, i32)) {
        let aux = ConfigureWindowAux::new()
            .width(width.max(1) as u32)
            .height(height.max(1) as u32);
        let _ = self.conn.configure_window(window, &aux);
        let _ = self.conn.flush();
    }

    /// Reads the replies' trailing events later, on the main loop: a reply can bring events
    /// along without the connection becoming readable again.
    fn pump_soon() {
        glib::idle_add_local_once(|| {
            if let Some(d) = DISPLAY.with_borrow(|d| d.clone()) {
                d.pump();
            }
        });
    }
}

/// What event handling needs for one editor window.
struct State {
    window: XWindow,
    view: RefCell<Option<ComPtr<IPlugView>>>,
    on_close: Box<dyn Fn()>,
    /// The size last given to the view with `onSize`.
    size: Cell<(i32, i32)>,
    resizable: bool,
    /// Where the window was when its tab was hidden.
    position: Cell<Option<(i16, i16)>>,
}

impl State {
    fn view(&self) -> Option<ComPtr<IPlugView>> {
        self.view.borrow().clone()
    }
}

/// The plugin's frame: resizes the window, and is the run loop the plugin's editor uses
/// (plugins query `IRunLoop` from the frame).
struct Frame {
    state: Rc<State>,
    display: Rc<Display>,
    handlers: RefCell<Vec<(usize, SourceId)>>,
    timers: RefCell<Vec<(usize, SourceId)>>,
}

impl Frame {
    /// Removes the sources the plugin left registered.
    fn clear(&self) {
        let sources = self.handlers.take().into_iter().chain(self.timers.take());
        for (_, id) in sources {
            remove_source(&id);
        }
    }
}

fn remove_source(id: &SourceId) {
    // Destroying through the context is safe even if GLib already dropped the source.
    if let Some(source) = MainContext::default().find_source_by_id(id) {
        source.destroy();
    }
}

impl Class for Frame {
    type Interfaces = (IPlugFrame, IRunLoop);
}

impl IPlugFrameTrait for Frame {
    unsafe fn resizeView(&self, view: *mut IPlugView, new_size: *mut ViewRect) -> tresult {
        if view.is_null() || new_size.is_null() {
            return kInvalidArgument;
        }
        let r = unsafe { *new_size };
        let size = (r.right - r.left, r.bottom - r.top);
        if size.0 <= 0 || size.1 <= 0 {
            return kInvalidArgument;
        }
        let window = self.state.window;
        if !self.state.resizable {
            let _ = self.display.fix_size(window, size);
        }
        self.display.resize(window, size);
        // The configure event arrives later with this size, which then needs no onSize.
        self.state.size.set(size);
        if let Some(view) = unsafe { ComRef::from_raw(view) } {
            let mut r = rect(size.0, size.1);
            unsafe { view.onSize(&mut r) };
        }
        kResultOk
    }
}

impl IRunLoopTrait for Frame {
    unsafe fn registerEventHandler(
        &self,
        handler: *mut IEventHandler,
        fd: FileDescriptor,
    ) -> tresult {
        // GLib's local sources must be added on the main context's thread.
        if !MainContext::default().is_owner() {
            return kResultFalse;
        }
        let Some(handler) = (unsafe { retain(handler) }) else {
            return kInvalidArgument;
        };
        let key = handler.as_ptr() as usize;
        let condition = IOCondition::IN | IOCondition::ERR | IOCondition::HUP;
        let id = glib::source::unix_fd_add_local(fd, condition, move |fd, _| {
            unsafe { handler.onFDIsSet(fd) };
            ControlFlow::Continue
        });
        self.handlers.borrow_mut().push((key, id));
        kResultTrue
    }

    unsafe fn unregisterEventHandler(&self, handler: *mut IEventHandler) -> tresult {
        unregister(&self.handlers, handler as usize)
    }

    unsafe fn registerTimer(
        &self,
        handler: *mut ITimerHandler,
        milliseconds: TimerInterval,
    ) -> tresult {
        if !MainContext::default().is_owner() {
            return kResultFalse;
        }
        let Some(handler) = (unsafe { retain(handler) }) else {
            return kInvalidArgument;
        };
        let key = handler.as_ptr() as usize;
        let interval = Duration::from_millis(milliseconds.max(1));
        let id = glib::timeout_add_local(interval, move || {
            unsafe { handler.onTimer() };
            ControlFlow::Continue
        });
        self.timers.borrow_mut().push((key, id));
        kResultTrue
    }

    unsafe fn unregisterTimer(&self, handler: *mut ITimerHandler) -> tresult {
        unregister(&self.timers, handler as usize)
    }
}

/// Removes every source registered for `handler`.
fn unregister(sources: &RefCell<Vec<(usize, SourceId)>>, handler: usize) -> tresult {
    let removed: Vec<SourceId> = {
        let mut sources = sources.borrow_mut();
        let (gone, kept) = sources.drain(..).partition(|(h, _)| *h == handler);
        *sources = kept;
        gone.into_iter().map(|(_, id)| id).collect()
    };
    if removed.is_empty() {
        return kInvalidArgument;
    }
    removed.iter().for_each(remove_source);
    kResultTrue
}

/// A new reference to a handler the plugin passed in; the plugin keeps its own.
unsafe fn retain<I: vst3::Interface>(ptr: *mut I) -> Option<ComPtr<I>> {
    unsafe { ComRef::from_raw(ptr) }.map(|r| r.to_com_ptr())
}

pub struct Window {
    display: Rc<Display>,
    state: Rc<State>,
    frame: ComWrapper<Frame>,
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
        let display = Display::get()?;
        let resizable = unsafe { view.canResize() } == kResultTrue;
        let size = view_size(&view).unwrap_or((400, 300));
        let window = display
            .create(title, size, resizable, parent)
            .map_err(|e| e.to_string())?;
        let state = Rc::new(State {
            window,
            view: RefCell::new(None),
            on_close,
            size: Cell::new(size),
            resizable,
            position: Cell::new(None),
        });
        display
            .windows
            .borrow_mut()
            .insert(window, Rc::downgrade(&state));
        let frame = ComWrapper::new(Frame {
            state: state.clone(),
            display: display.clone(),
            handlers: RefCell::default(),
            timers: RefCell::default(),
        });
        let frame_ptr = frame
            .as_com_ref::<IPlugFrame>()
            .map_or(std::ptr::null_mut(), |r| r.as_ptr());
        unsafe { view.setFrame(frame_ptr) };

        let attached = unsafe { view.attached(window as usize as *mut c_void, PLATFORM_TYPE) };
        if attached != kResultOk {
            unsafe { view.setFrame(std::ptr::null_mut()) };
            frame.clear();
            display.windows.borrow_mut().remove(&window);
            let _ = display.conn.destroy_window(window);
            let _ = display.conn.flush();
            return Err("the plugin didn't accept the window".into());
        }
        *state.view.borrow_mut() = Some(view.clone());
        if let Some(now) = view_size(&view).filter(|&s| s != size) {
            if !resizable {
                let _ = display.fix_size(window, now);
            }
            display.resize(window, now);
            state.size.set(now);
        }
        let _ = display.conn.map_window(window);
        let _ = display.conn.flush();
        Display::pump_soon();
        Ok(Window {
            display,
            state,
            frame,
        })
    }

    fn map(&self) {
        let (c, window) = (&self.display.conn, self.state.window);
        let _ = c.map_window(window);
        if let Some((x, y)) = self.state.position.take() {
            let aux = ConfigureWindowAux::new().x(i32::from(x)).y(i32::from(y));
            let _ = c.configure_window(window, &aux);
        }
    }
}

impl EditorWindow for Window {
    fn front(&self) {
        let (d, window) = (&self.display, self.state.window);
        self.map();
        let raise = ConfigureWindowAux::new().stack_mode(StackMode::ABOVE);
        let _ = d.conn.configure_window(window, &raise);
        // Ask the window manager to activate it (source 1: an application).
        let activate = ClientMessageEvent::new(
            32,
            window,
            d.atoms._NET_ACTIVE_WINDOW,
            [1, x11rb::CURRENT_TIME, 0, 0, 0],
        );
        let mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
        let _ = d.conn.send_event(false, d.root, mask, activate);
        let _ = d.conn.flush();
    }

    fn set_visible(&self, visible: bool) {
        let (d, window) = (&self.display, self.state.window);
        if visible {
            self.map();
        } else {
            // Unmapped windows may be placed afresh by the window manager: remember where
            // this one was.
            let at = d
                .conn
                .translate_coordinates(window, d.root, 0, 0)
                .ok()
                .and_then(|c| c.reply().ok());
            self.state.position.set(at.map(|t| (t.dst_x, t.dst_y)));
            let _ = d.conn.unmap_window(window);
            Display::pump_soon();
        }
        let _ = d.conn.flush();
    }

    fn close(self) {
        let view = self.state.view.borrow_mut().take();
        if let Some(view) = view {
            detach(&view);
        }
        self.frame.clear();
        let window = self.state.window;
        self.display.windows.borrow_mut().remove(&window);
        let _ = self.display.conn.destroy_window(window);
        let _ = self.display.conn.flush();
    }
}
