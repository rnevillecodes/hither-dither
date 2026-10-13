//! A file's effect chain: the effect slots, their order and bypass, and the chain's latency.
//!
//! A [`Slot`] is one live effect instance. The undo history refers to slots by [`SlotId`], so
//! undoing a removal brings back the same instance with its settings. A [`Chain`] is an
//! immutable snapshot of the slots in processing order plus Bypass All, as the playback feeder
//! and the save render see it.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use vst3::Steinberg::Vst::SpeakerArr;

use crate::plugins::ClassId;
use crate::plugins::instance::{Instance, speaker_arrangement};
use crate::plugins::main_thread::UiThread;
use crate::plugins::scan::ScannedClass;

/// Identifies an effect slot, unique for the life of the app.
pub type SlotId = u32;

/// Most effects a chain can hold.
pub const MAX_EFFECTS: usize = 8;

/// Largest block the live chain processes: the playback feeder's chunk size.
pub const LIVE_BLOCK: usize = 1024;

/// One effect in a chain, or kept by the undo history.
///
/// The plugin instance is always released on the UI thread, whichever thread drops the last
/// reference to the slot.
pub struct Slot {
    id: SlotId,
    class_id: ClassId,
    name: String,
    vendor: String,
    version: String,
    has_editor: bool,
    /// Taken by `Drop`, which hands it to the UI thread.
    instance: Option<Instance>,
    ui: Arc<dyn UiThread>,
    latency: AtomicU32,
    /// The slot's own bypass switch.
    bypassed: AtomicBool,
    /// Whether the slot should currently pass audio through: its own switch or Bypass All.
    /// Set when its chain is built; see [`Chain::apply_bypass`].
    wanted_bypass: AtomicBool,
    /// The value last sent to the plugin's bypass parameter.
    applied_bypass: AtomicBool,
    /// The plugin's bypass parameter may hold anything (its state was just restored), so
    /// the next [`Slot::apply_bypass`] sends the wanted value even if it seems unchanged.
    bypass_unknown: AtomicBool,
}

impl Slot {
    /// Wraps a configured instance of `class` that reported `latency` frames.
    pub fn new(
        id: SlotId,
        class: &ScannedClass,
        instance: Instance,
        latency: u32,
        ui: Arc<dyn UiThread>,
    ) -> Slot {
        Slot {
            id,
            class_id: class.class_id,
            name: class.name.clone(),
            vendor: class.vendor.clone(),
            version: class.version.clone(),
            has_editor: class.has_editor,
            instance: Some(instance),
            ui,
            latency: AtomicU32::new(latency),
            bypassed: AtomicBool::new(false),
            wanted_bypass: AtomicBool::new(false),
            applied_bypass: AtomicBool::new(false),
            bypass_unknown: AtomicBool::new(false),
        }
    }

    pub fn id(&self) -> SlotId {
        self.id
    }

    pub fn class_id(&self) -> ClassId {
        self.class_id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn vendor(&self) -> &str {
        &self.vendor
    }

    /// The plugin's version, as scanned.
    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn instance(&self) -> &Instance {
        // Only `Drop` takes it.
        self.instance.as_ref().expect("slot instance")
    }

    pub fn latency(&self) -> u32 {
        self.latency.load(Ordering::Relaxed)
    }

    pub fn set_latency(&self, frames: u32) {
        self.latency.store(frames, Ordering::Relaxed);
    }

    /// The slot's own bypass switch (not Bypass All).
    pub fn bypassed(&self) -> bool {
        self.bypassed.load(Ordering::Relaxed)
    }

    pub fn set_bypassed(&self, on: bool) {
        self.bypassed.store(on, Ordering::Relaxed);
    }

    /// Makes the next [`Chain::apply_bypass`] send this slot's bypass state to the plugin
    /// whatever was sent before, after restoring a state that may contain any value.
    pub fn resend_bypass(&self) {
        self.bypass_unknown.store(true, Ordering::Relaxed);
    }

    /// Whether the host leaves this slot out of processing: it is bypassed and the plugin
    /// has no bypass parameter to do it itself.
    pub fn skipped(&self) -> bool {
        self.wanted_bypass.load(Ordering::Relaxed) && self.instance().bypass_param().is_none()
    }

    /// Sends the wanted bypass state to the plugin's bypass parameter if it changed. UI thread.
    fn apply_bypass(&self) {
        let Some(param) = self.instance().bypass_param() else {
            return;
        };
        let want = self.wanted_bypass.load(Ordering::Relaxed);
        let unknown = self.bypass_unknown.swap(false, Ordering::Relaxed);
        if self.applied_bypass.swap(want, Ordering::Relaxed) != want || unknown {
            self.instance()
                .set_param(param, if want { 1.0 } else { 0.0 });
        }
    }

    fn info(&self) -> EffectInfo {
        EffectInfo {
            slot_id: self.id,
            class_id: self.class_id,
            name: self.name.clone(),
            vendor: self.vendor.clone(),
            has_editor: self.has_editor,
            bypassed: self.bypassed(),
        }
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        let Some(instance) = self.instance.take() else {
            return;
        };
        if self.ui.is_current() {
            drop(instance);
            return;
        }
        let cell = Arc::new(Mutex::new(Some(instance)));
        let posted = cell.clone();
        let sent = self.ui.post(Box::new(move || {
            drop(posted.lock().unwrap_or_else(|e| e.into_inner()).take());
        }));
        if sent.is_err() {
            // The event loop has gone (the app is quitting): releasing the plugin off the UI
            // thread could crash it, so leave it to the process exit.
            std::mem::forget(cell.lock().unwrap_or_else(|e| e.into_inner()).take());
        }
    }
}

/// The slots of a chain in processing order, with Bypass All.
#[derive(Clone, Default)]
pub struct Chain {
    pub slots: Vec<Arc<Slot>>,
    pub bypass_all: bool,
}

impl Chain {
    /// Builds a chain and records each slot's wanted bypass state.
    pub fn new(slots: Vec<Arc<Slot>>, bypass_all: bool) -> Chain {
        for s in &slots {
            s.wanted_bypass
                .store(bypass_all || s.bypassed(), Ordering::Relaxed);
        }
        Chain { slots, bypass_all }
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Whether the chain changes the audio: some effect isn't bypassed and Bypass All is off.
    pub fn is_active(&self) -> bool {
        !self.bypass_all && self.slots.iter().any(|s| !s.bypassed())
    }

    /// The slots that process audio, in order. Bypassed slots with a bypass parameter still
    /// process, bypassing themselves, so their latency stays and they don't click.
    pub fn processed(&self) -> impl Iterator<Item = &Arc<Slot>> {
        self.slots.iter().filter(|s| !s.skipped())
    }

    /// Total latency of the processed slots, in frames.
    pub fn latency(&self) -> u32 {
        self.processed().map(|s| s.latency()).sum()
    }

    /// Sends each slot's bypass state to plugins that bypass themselves. UI thread.
    pub fn apply_bypass(&self) {
        for s in &self.slots {
            s.apply_bypass();
        }
    }

    pub fn info(&self) -> ChainInfo {
        ChainInfo {
            effects: self.slots.iter().map(|s| s.info()).collect(),
            bypass_all: self.bypass_all,
            latency: self.latency(),
        }
    }
}

/// A chain as the frontend shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainInfo {
    pub effects: Vec<EffectInfo>,
    pub bypass_all: bool,
    /// Total processing delay in frames, compensated in playback and saving.
    pub latency: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectInfo {
    pub slot_id: SlotId,
    pub class_id: ClassId,
    pub name: String,
    pub vendor: String,
    pub has_editor: bool,
    pub bypassed: bool,
}

/// A file's channel layout as named in messages, such as "stereo" or "5.1".
pub fn layout_name(channels: u16, mask: Option<u32>) -> String {
    match speaker_arrangement(channels, mask) {
        SpeakerArr::kMono => "mono".into(),
        SpeakerArr::kStereo => "stereo".into(),
        SpeakerArr::k40Music => "quad".into(),
        SpeakerArr::k50 => "5.0".into(),
        SpeakerArr::k51 => "5.1".into(),
        SpeakerArr::k71Music => "7.1".into(),
        _ => format!("{channels}-channel"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_are_named() {
        assert_eq!(layout_name(1, None), "mono");
        assert_eq!(layout_name(2, Some(0x3)), "stereo");
        assert_eq!(layout_name(4, Some(0x33)), "quad");
        assert_eq!(layout_name(6, Some(0x3F)), "5.1");
        assert_eq!(layout_name(6, Some(0x60F)), "5.1");
        assert_eq!(layout_name(8, Some(0x63F)), "7.1");
        assert_eq!(layout_name(3, None), "3-channel");
    }
}
