//! Integrated loudness (ITU-R BS.1770-4 / EBU R128) via `ebur128`.

use ebur128::{Channel, EbuR128, Mode};
use serde::Serialize;

use crate::wav::WavInfo;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Loudness {
    Value { lufs: f64 },
    /// Every gating block is below the absolute gate (-70 LUFS): shown as -inf.
    Silent,
    Unavailable { reason: String },
}

pub struct LoudnessMeter {
    meter: Result<EbuR128, String>,
}

/// Ranges shorter than one gating block cannot be measured.
const MIN_RANGE_MS: u64 = 400;

impl LoudnessMeter {
    pub fn new(info: &WavInfo, frames: u64) -> Self {
        Self {
            meter: Self::create(info, frames),
        }
    }

    fn create(info: &WavInfo, frames: u64) -> Result<EbuR128, String> {
        let map: &[Channel] = match (info.channels, info.channel_mask) {
            (1, _) => &[Channel::Center],
            (2, _) => &[Channel::Left, Channel::Right],
            // 5.1 in WAV order; LFE is excluded per BS.1770.
            (6, None | Some(0x3F) | Some(0x60F)) => &[
                Channel::Left,
                Channel::Right,
                Channel::Center,
                Channel::Unused,
                Channel::LeftSurround,
                Channel::RightSurround,
            ],
            _ => {
                return Err(
                    "Loudness is only measured for mono, stereo and 5.1 channel layouts".into(),
                );
            }
        };
        if frames * 1000 < MIN_RANGE_MS * info.sample_rate as u64 {
            return Err("Loudness needs at least 400 ms of audio".into());
        }
        let mut m = EbuR128::new(info.channels as u32, info.sample_rate, Mode::I)
            .map_err(|_| format!("Loudness cannot be measured at {} Hz", info.sample_rate))?;
        m.set_channel_map(map)
            .map_err(|e| format!("Loudness meter error: {e:?}"))?;
        Ok(m)
    }

    pub fn add(&mut self, interleaved: &[f32]) {
        if let Ok(m) = &mut self.meter
            && let Err(e) = m.add_frames_f32(interleaved)
        {
            self.meter = Err(format!("Loudness meter error: {e:?}"));
        }
    }

    pub fn finish(&self) -> Loudness {
        match &self.meter {
            Err(reason) => Loudness::Unavailable {
                reason: reason.clone(),
            },
            Ok(m) => match m.loudness_global() {
                Ok(l) if l.is_finite() => Loudness::Value { lufs: l },
                Ok(_) => Loudness::Silent,
                Err(e) => Loudness::Unavailable {
                    reason: format!("Loudness meter error: {e:?}"),
                },
            },
        }
    }
}
