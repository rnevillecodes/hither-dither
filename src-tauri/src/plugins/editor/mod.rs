//! Native plugin editor windows.
//!
//! Each effect slot can show its plugin's own editor in a top-level window that the host
//! creates and that stays above the main window (see the per-OS modules). [`Editors`] keeps at
//! most one window per slot and hides and shows them with their tab.
//!
//! Everything here runs on the UI thread: the public methods hop there through
//! [`main_thread::run`]. Platform windows call back (when the user closes one) synchronously,
//! so the manager's lock is never held while calling into a window or a plugin.

#[cfg(target_os = "macos")]
mod mac;
#[cfg(target_os = "macos")]
use mac as platform;
#[cfg(windows)]
mod win;
#[cfg(windows)]
use win as platform;
#[cfg(all(unix, not(target_os = "macos")))]
mod x11;
#[cfg(all(unix, not(target_os = "macos")))]
use x11 as platform;

pub use platform::{Parent, parent_of};

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, Weak};

use serde::Serialize;
use vst3::ComPtr;
use vst3::Steinberg::Vst::{IEditControllerTrait, ViewType};
use vst3::Steinberg::{IPlugView, IPlugViewTrait, kResultTrue};

use super::cache::{LoadStep, PluginCache};
use super::main_thread::{self, Closed, UiThread};
use crate::fx::chain::{Slot, SlotId};
use crate::state::FileId;

/// Shown with the generic controls when a plugin has no editor that can be shown here.
const NO_EDITOR: &str = "This effect has no editor of its own on this system.";

/// What opening an effect's controls did.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorOpened {
    /// Whether the plugin's own editor window is showing. Otherwise the frontend shows the
    /// generic controls.
    pub native: bool,
    /// Why the plugin's own editor isn't showing, when it was asked for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl EditorOpened {
    fn generic(reason: Option<String>) -> EditorOpened {
        EditorOpened {
            native: false,
            reason,
        }
    }
}

/// An editor window; `close` detaches the view and destroys the window.
trait EditorWindow {
    fn front(&self);
    fn set_visible(&self, visible: bool);
    fn close(self);
}

type Key = (FileId, SlotId);

struct Editor {
    window: platform::Window,
    /// Keeps the plugin alive until its view is gone.
    _slot: Arc<Slot>,
    visible: bool,
}

/// Only touched on the UI thread.
struct UiOnly<T>(T);

// SAFETY: editors are created, used and dropped on the UI thread only; the map holding them
// is shared so that any thread can ask the UI thread to act on them.
unsafe impl<T> Send for UiOnly<T> {}

#[derive(Default)]
struct Inner {
    open: HashMap<Key, UiOnly<Editor>>,
    /// The tab whose editors are showing; `None` until the frontend says.
    active: Option<FileId>,
}

type ParentFn = Box<dyn Fn() -> Option<Parent> + Send + Sync>;
type ClosedFn = Box<dyn Fn(FileId, SlotId) + Send + Sync>;

/// The open editor windows.
pub struct Editors {
    ui: Arc<dyn UiThread>,
    /// The main window, which editors stay above. Called on the UI thread.
    parent: ParentFn,
    /// Told when an editor window closes, for whatever reason. Called on the UI thread.
    on_closed: ClosedFn,
    inner: Mutex<Inner>,
}

impl Editors {
    pub fn new(
        ui: Arc<dyn UiThread>,
        parent: impl Fn() -> Option<Parent> + Send + Sync + 'static,
        on_closed: impl Fn(FileId, SlotId) + Send + Sync + 'static,
    ) -> Editors {
        Editors {
            ui,
            parent: Box::new(parent),
            on_closed: Box::new(on_closed),
            inner: Mutex::default(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Shows `slot`'s editor, titled `title`, or brings it to the front if it is already
    /// open. Reports `native: false` with the reason when the plugin's editor can't be shown.
    pub fn open(
        self: &Arc<Self>,
        file_id: FileId,
        slot: Arc<Slot>,
        title: String,
        cache: Arc<PluginCache>,
    ) -> Result<EditorOpened, Closed> {
        let this = self.clone();
        main_thread::run(&*self.ui, move || {
            this.open_on_ui(file_id, slot, &title, &cache)
        })
    }

    fn open_on_ui(
        self: &Arc<Self>,
        file_id: FileId,
        slot: Arc<Slot>,
        title: &str,
        cache: &PluginCache,
    ) -> EditorOpened {
        let key = (file_id, slot.id());
        // Taken out while the window comes forward, so a callback can't find the lock held.
        let existing = self.lock().open.remove(&key);
        if let Some(mut editor) = existing {
            editor.0.window.front();
            editor.0.visible = true;
            self.lock().open.insert(key, editor);
            return EditorOpened {
                native: true,
                reason: None,
            };
        }
        if let Err(reason) = platform::available() {
            return EditorOpened::generic(Some(reason));
        }
        let Some(controller) = slot.instance().controller() else {
            return EditorOpened::generic(Some(NO_EDITOR.into()));
        };

        let _loading = cache.loading(slot.class_id(), LoadStep::OpenEditor);
        let view = unsafe { controller.createView(ViewType::kEditor) };
        let Some(view) = (unsafe { ComPtr::<IPlugView>::from_raw(view) }) else {
            return EditorOpened::generic(Some(NO_EDITOR.into()));
        };
        if unsafe { view.isPlatformTypeSupported(platform::PLATFORM_TYPE) } != kResultTrue {
            return EditorOpened::generic(Some(NO_EDITOR.into()));
        }
        let weak = Arc::downgrade(self);
        let on_close = move || {
            if let Some(editors) = Weak::upgrade(&weak) {
                editors.close_on_ui(key);
            }
        };
        let parent = (self.parent)();
        match platform::Window::open(parent, title, view, Box::new(on_close)) {
            Ok(window) => {
                let editor = Editor {
                    window,
                    _slot: slot,
                    visible: true,
                };
                self.lock().open.insert(key, UiOnly(editor));
                EditorOpened {
                    native: true,
                    reason: None,
                }
            }
            Err(e) => EditorOpened::generic(Some(format!(
                "The effect's editor couldn't be opened ({e}), so its controls are shown instead."
            ))),
        }
    }

    /// Closes the editor of `slot` in file `file_id`, if open.
    pub fn close(self: &Arc<Self>, file_id: FileId, slot: SlotId) -> Result<(), Closed> {
        let this = self.clone();
        main_thread::run(&*self.ui, move || this.close_on_ui((file_id, slot)))
    }

    fn close_on_ui(&self, key: Key) {
        let editor = self.lock().open.remove(&key);
        if let Some(editor) = editor {
            editor.0.window.close();
            (self.on_closed)(key.0, key.1);
        }
    }

    /// Closes file `file_id`'s editors except those of the slots in `keep`: those of effects
    /// that left the chain, or all of them when the file closes or a save empties its chain.
    pub fn retain(self: &Arc<Self>, file_id: FileId, keep: Vec<SlotId>) -> Result<(), Closed> {
        let this = self.clone();
        main_thread::run(&*self.ui, move || {
            let gone: Vec<Key> = this
                .lock()
                .open
                .keys()
                .filter(|&&(f, s)| f == file_id && !keep.contains(&s))
                .copied()
                .collect();
            for key in gone {
                this.close_on_ui(key);
            }
        })
    }

    /// Shows the editors of file `file_id` and hides the others, which come back where they
    /// were when their tab is active again. `None` hides them all.
    pub fn set_active_file(self: &Arc<Self>, file_id: Option<FileId>) -> Result<(), Closed> {
        let this = self.clone();
        main_thread::run(&*self.ui, move || {
            let mut changed = Vec::new();
            {
                let mut inner = this.lock();
                inner.active = file_id;
                let keys: Vec<Key> = inner.open.keys().copied().collect();
                for key in keys {
                    let show = Some(key.0) == file_id;
                    if inner.open.get(&key).is_some_and(|e| e.0.visible != show)
                        && let Some(e) = inner.open.remove(&key)
                    {
                        changed.push((key, e, show));
                    }
                }
            }
            for (key, mut editor, show) in changed {
                editor.0.window.set_visible(show);
                editor.0.visible = show;
                this.lock().open.insert(key, editor);
            }
        })
    }

    /// Closes every editor, when the app quits.
    pub fn close_all(self: &Arc<Self>) {
        let this = self.clone();
        let _ = main_thread::run(&*self.ui, move || {
            let keys: Vec<Key> = this.lock().open.keys().copied().collect();
            for key in keys {
                this.close_on_ui(key);
            }
        });
    }

    /// Whether `slot` of file `file_id` has its editor open.
    pub fn is_open(&self, file_id: FileId, slot: SlotId) -> bool {
        self.lock().open.contains_key(&(file_id, slot))
    }
}

/// Detaches `view` from its window before the window goes. UI thread.
fn detach(view: &ComPtr<IPlugView>) {
    unsafe {
        view.removed();
        view.setFrame(std::ptr::null_mut());
    }
}

/// The view's size as width and height.
fn view_size(view: &ComPtr<IPlugView>) -> Option<(i32, i32)> {
    let mut rect = vst3::Steinberg::ViewRect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    let ok = unsafe { view.getSize(&mut rect) } == vst3::Steinberg::kResultOk;
    let size = (rect.right - rect.left, rect.bottom - rect.top);
    (ok && size.0 > 0 && size.1 > 0).then_some(size)
}

fn rect(width: i32, height: i32) -> vst3::Steinberg::ViewRect {
    vst3::Steinberg::ViewRect {
        left: 0,
        top: 0,
        right: width,
        bottom: height,
    }
}
