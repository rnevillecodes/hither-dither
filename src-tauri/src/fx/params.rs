//! Keeping controllers and generic controls in step with what plugins report.
//!
//! Parameter changes made in a plugin's own editor, and output parameter values from its
//! processor (such as meters), arrive on whichever thread the plugin calls from. [`ParamSync`]
//! collects them per slot and, at most once every [`INTERVAL`], hands the output values to the
//! controller on the UI thread and reports the editor changes, with their display strings, to
//! a listener (the `fx-param-changed` event). A worker thread does this while there is
//! anything to do, and stops after a second without.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};
use std::time::Duration;

use serde::Serialize;

use super::chain::{Slot, SlotId};
use crate::plugins::host::PendingParams;
use crate::plugins::main_thread::{self, UiThread};
use crate::state::FileId;

/// Shortest time between two updates of the same slot: 30 Hz.
pub const INTERVAL: Duration = Duration::from_millis(33);

/// How long the worker waits for something to do before it stops.
const IDLE: Duration = Duration::from_secs(1);

/// One parameter's new value.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParamChange {
    pub id: u32,
    /// Normalised (0 to 1).
    pub value: f64,
    /// As the plugin shows it, without the units.
    pub display: String,
}

/// Parameters of one effect changed in its own editor.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParamsChanged {
    pub file_id: FileId,
    pub slot_id: SlotId,
    pub changes: Vec<ParamChange>,
}

pub type ChangedFn = Arc<dyn Fn(ParamsChanged) + Send + Sync>;
pub type ResetFn = Arc<dyn Fn(FileId, SlotId) + Send + Sync>;

struct Watched {
    file_id: FileId,
    slot: Weak<Slot>,
    /// Values set in the plugin's editor since the last update.
    edited: PendingParams,
}

#[derive(Default)]
struct Inner {
    watched: HashMap<SlotId, Arc<Watched>>,
    /// Slots with something to update.
    dirty: HashSet<SlotId>,
    worker: bool,
}

pub struct ParamSync {
    ui: Arc<dyn UiThread>,
    inner: Mutex<Inner>,
    wake: Condvar,
    on_changed: Mutex<Option<ChangedFn>>,
    on_reset: Mutex<Option<ResetFn>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl ParamSync {
    pub fn new(ui: Arc<dyn UiThread>) -> ParamSync {
        ParamSync {
            ui,
            inner: Mutex::default(),
            wake: Condvar::new(),
            on_changed: Mutex::default(),
            on_reset: Mutex::default(),
        }
    }

    /// Sets what to call with changes made in plugins' own editors. Called on the worker
    /// thread.
    pub fn on_changed(&self, f: impl Fn(ParamsChanged) + Send + Sync + 'static) {
        *lock(&self.on_changed) = Some(Arc::new(f));
    }

    /// Sets what to call when a plugin says its parameters' values or titles all changed.
    /// Called on whichever thread the plugin calls from, usually the UI thread.
    pub fn on_reset(&self, f: impl Fn(FileId, SlotId) + Send + Sync + 'static) {
        *lock(&self.on_reset) = Some(Arc::new(f));
    }

    /// Starts following `slot`, an effect of file `file_id`.
    pub fn watch(&self, file_id: FileId, slot: &Arc<Slot>) {
        let mut inner = lock(&self.inner);
        inner.watched.retain(|_, w| w.slot.strong_count() > 0);
        let watched = Watched {
            file_id,
            slot: Arc::downgrade(slot),
            edited: PendingParams::default(),
        };
        inner.watched.insert(slot.id(), Arc::new(watched));
    }

    /// Parameter `id` of `slot` was set to `value` in the plugin's editor.
    pub fn edited(self: &Arc<Self>, slot: SlotId, id: u32, value: f64) {
        let watched = lock(&self.inner).watched.get(&slot).cloned();
        if let Some(w) = watched {
            w.edited.set(id, value);
            self.mark(slot);
        }
    }

    /// `slot`'s processor reported output parameter values.
    pub fn output(self: &Arc<Self>, slot: SlotId) {
        self.mark(slot);
    }

    /// `slot`'s plugin says its parameters' values or titles changed.
    pub fn reset(&self, slot: SlotId) {
        let file_id = lock(&self.inner).watched.get(&slot).map(|w| w.file_id);
        let f = lock(&self.on_reset).clone();
        if let (Some(file_id), Some(f)) = (file_id, f) {
            f(file_id, slot);
        }
    }

    fn mark(self: &Arc<Self>, slot: SlotId) {
        let mut inner = lock(&self.inner);
        inner.dirty.insert(slot);
        if inner.worker {
            self.wake.notify_one();
            return;
        }
        inner.worker = true;
        let weak = Arc::downgrade(self);
        let spawned = std::thread::Builder::new()
            .name("fx-params".into())
            .spawn(move || work(weak));
        if spawned.is_err() {
            inner.worker = false;
        }
    }

    /// The watched slots with something to update, waiting up to [`IDLE`] for one. `None`
    /// when there is still nothing, which stops the worker.
    fn next_batch(&self) -> Option<Vec<Arc<Watched>>> {
        let mut inner = lock(&self.inner);
        if inner.dirty.is_empty() {
            inner = self
                .wake
                .wait_timeout(inner, IDLE)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
        if inner.dirty.is_empty() {
            inner.worker = false;
            return None;
        }
        let dirty = std::mem::take(&mut inner.dirty);
        Some(
            dirty
                .iter()
                .filter_map(|s| inner.watched.get(s).cloned())
                .collect(),
        )
    }

    /// Updates `batch` on the UI thread, then reports the editor changes.
    fn update(&self, batch: Vec<Arc<Watched>>) {
        let events = main_thread::run(&*self.ui, move || {
            let mut events = Vec::new();
            for w in batch {
                // Upgraded and released here, so a slot never ends off the UI thread.
                let Some(slot) = w.slot.upgrade() else {
                    continue;
                };
                let instance = slot.instance();
                instance.apply_outputs();
                let changes: Vec<ParamChange> = w
                    .edited
                    .take()
                    .into_iter()
                    .map(|(id, value)| ParamChange {
                        id,
                        value,
                        display: instance.display(id, value),
                    })
                    .collect();
                if !changes.is_empty() {
                    events.push(ParamsChanged {
                        file_id: w.file_id,
                        slot_id: slot.id(),
                        changes,
                    });
                }
            }
            events
        });
        let f = lock(&self.on_changed).clone();
        if let (Ok(events), Some(f)) = (events, f) {
            for e in events {
                f(e);
            }
        }
    }
}

fn work(sync: Weak<ParamSync>) {
    loop {
        let Some(this) = sync.upgrade() else { return };
        let Some(batch) = this.next_batch() else {
            return;
        };
        this.update(batch);
        drop(this);
        std::thread::sleep(INTERVAL);
    }
}
