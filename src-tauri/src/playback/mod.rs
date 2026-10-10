//! Audio playback of the loaded file.

mod device;
mod engine;
mod mapping;

pub use device::DeviceInfo;
pub use engine::{PlayRequest, Shared};
pub use mapping::ChannelMap;

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;

use crate::wav::SampleSource;
use device::AudioThread;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PlaybackInfo {
    pub available: bool,
    /// Why playback is unavailable.
    pub reason: Option<String>,
    pub device: Option<DeviceInfo>,
}

struct Session {
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
        src: Arc<SampleSource>,
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
            src,
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
        *self.session.lock().unwrap() = Some(Session {
            shared: shared.clone(),
            feeder,
        });
        Ok(shared)
    }

    /// Stop playback; returns the audible frame at the moment it stopped.
    pub fn stop(&self) -> Option<f64> {
        let session = self.session.lock().unwrap().take()?;
        Some(self.end(session))
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

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}
