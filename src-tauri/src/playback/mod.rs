//! Audio playback of an open file.

mod device;
mod engine;
mod mapping;

pub use device::DeviceInfo;
pub use engine::{PlayRequest, Retarget, Shared};
pub use mapping::ChannelMap;

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;

use crate::edit::{Audio, Located};
use device::AudioThread;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PlaybackInfo {
    pub available: bool,
    /// Why playback is unavailable.
    pub reason: Option<String>,
    pub device: Option<DeviceInfo>,
}

struct Session {
    /// The audio being played (the latest switched to), so closing its file can stop
    /// playback and edits can follow it.
    source: Arc<Audio>,
    /// The play range in `source`.
    range: (u64, u64),
    shared: Arc<Shared>,
    feeder: std::thread::JoinHandle<()>,
}

pub struct Player {
    info: PlaybackInfo,
    audio: Option<AudioThread>,
    volume: Arc<AtomicU32>,
    session: Mutex<Option<Session>>,
}

impl Player {
    /// Probe the default output device. Without one, playback stays disabled.
    pub fn new() -> Self {
        let info = match device::probe() {
            Ok(d) => PlaybackInfo {
                available: true,
                reason: None,
                device: Some(d),
            },
            Err(reason) => PlaybackInfo {
                available: false,
                reason: Some(reason),
                device: None,
            },
        };
        Self {
            audio: info.available.then(AudioThread::spawn),
            info,
            volume: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            session: Mutex::new(None),
        }
    }

    pub fn info(&self) -> PlaybackInfo {
        self.info.clone()
    }

    /// Start playing `req`, replacing any current playback.
    pub fn play(
        &self,
        src: Arc<Audio>,
        req: PlayRequest,
        looping: bool,
    ) -> Result<Arc<Shared>, String> {
        let audio = self
            .audio
            .as_ref()
            .ok_or_else(|| self.info.reason.clone().unwrap_or_default())?;
        self.stop();
        let info = src.info();
        let format = audio.prepare(info.sample_rate)?;
        let five_one =
            info.channels == 6 && matches!(info.channel_mask, None | Some(0x3F) | Some(0x60F));
        let map = ChannelMap::new(info.channels as usize, format.channels as usize, five_one);
        let (engine, feeder) = engine::start(
            src.clone(),
            req,
            looping,
            map,
            format.sample_rate,
            self.volume.clone(),
        );
        let shared = engine.shared().clone();
        engine.wait_prefilled(std::time::Duration::from_millis(150));
        if let Err(e) = audio.start(engine, format) {
            shared.stop.store(true, Ordering::Relaxed);
            let _ = feeder.join();
            return Err(e);
        }
        let range = shared.range();
        *self.session.lock().unwrap() = Some(Session {
            source: src,
            range,
            shared: shared.clone(),
            feeder,
        });
        Ok(shared)
    }

    /// If `audio`'s file is playing, switch playback to this edit state of it: playing
    /// continues from the same source audio (or the join that replaced it), and the play
    /// range is mapped the same way. A range covering the whole file stays the whole file.
    pub fn follow(&self, audio: &Arc<Audio>) {
        let mut guard = self.session.lock().unwrap();
        let Some(s) = guard
            .as_mut()
            .filter(|s| s.source.same_source(audio) && !Arc::ptr_eq(&s.source, audio))
        else {
            return;
        };
        s.range = follow_range(&s.source, audio, s.range);
        s.source = audio.clone();
        s.shared.retarget(Retarget {
            audio: audio.clone(),
            start: s.range.0,
            end: s.range.1,
        });
    }

    /// Play `[start, end)` of what is playing now, from where it is.
    pub fn set_play_range(&self, start: u64, end: u64) {
        let mut guard = self.session.lock().unwrap();
        let Some(s) = guard.as_mut() else { return };
        let end = end.min(s.source.frames());
        s.range = (start.min(end), end);
        s.shared.retarget(Retarget {
            audio: s.source.clone(),
            start: s.range.0,
            end: s.range.1,
        });
    }

    /// Stop playback; returns the audible frame at the moment it stopped.
    pub fn stop(&self) -> Option<f64> {
        let session = self.session.lock().unwrap().take()?;
        Some(self.end(session))
    }

    /// Stop playback if it is playing any version of `src`'s file.
    pub fn stop_if_playing(&self, src: &Audio) {
        let mut guard = self.session.lock().unwrap();
        if guard.as_ref().is_some_and(|s| s.source.same_source(src)) {
            let session = guard.take().unwrap();
            drop(guard);
            self.end(session);
        }
    }

    /// Stop playback only if `shared` is still the current session (releases the
    /// output device after playback reaches its end without racing a newer `play`).
    pub fn finish(&self, shared: &Arc<Shared>) {
        let mut guard = self.session.lock().unwrap();
        if guard
            .as_ref()
            .is_some_and(|s| Arc::ptr_eq(&s.shared, shared))
        {
            let session = guard.take().unwrap();
            drop(guard);
            self.end(session);
        }
    }

    fn end(&self, session: Session) -> f64 {
        let pos = session.shared.position();
        session.shared.stop.store(true, Ordering::Relaxed);
        if let Some(audio) = &self.audio {
            audio.stop();
        }
        let _ = session.feeder.join();
        pos
    }

    pub fn set_volume(&self, gain: f32) {
        self.volume
            .store(gain.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    pub fn set_looping(&self, on: bool) {
        if let Some(s) = self.session.lock().unwrap().as_ref() {
            s.shared.looping.store(on, Ordering::Relaxed);
        }
    }
}

/// The play range `[start, end)` of `old` as it appears in `new` (same source, other
/// edits). Ends that were removed snap to the audio that followed them.
pub fn follow_range(old: &Audio, new: &Audio, (start, end): (u64, u64)) -> (u64, u64) {
    let frames = new.frames();
    if start == 0 && end >= old.frames() {
        return (0, frames);
    }
    let (from, to) = (old.edits(), new.edits());
    let place = |f: Option<u64>| f.and_then(|f| from.source_at(f)).map(|s| to.output_of(s));
    let new_start = match place(Some(start)) {
        Some(Located::At(out)) => out,
        Some(Located::Removed { next }) => next.unwrap_or(frames),
        None => frames,
    };
    // Map the last frame played, rather than the exclusive end.
    let new_end = match place(end.checked_sub(1)) {
        Some(Located::At(out)) => out + 1,
        Some(Located::Removed { next }) => next.unwrap_or(frames),
        None => frames,
    };
    (new_start.min(new_end), new_end)
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}
