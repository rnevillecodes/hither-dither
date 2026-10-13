//! Offline rendering of a chain, for Save.
//!
//! Save reuses the chain's live instances: [`offline`] reconfigures them for offline rendering,
//! and [`realtime`] puts them back if the save fails. A [`Render`] is the edited audio run
//! through the chain with its latency removed, quantised as the file stores it, for the writer.
//! It analyses the same quantised frames as it goes, so the saved file's statistics and peaks
//! come from the one pass.

use std::cell::RefCell;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::chain::{Chain, LIVE_BLOCK, Slot};
use super::live::{Live, LiveChain};
use crate::edit::Audio;
use crate::peaks::{PeakPyramid, PyramidBuilder};
use crate::plugins::instance::Config;
use crate::plugins::main_thread::{self, UiThread};
use crate::stats::{Accumulator, Analysis};
use crate::wav::{Frames, NativeFrames, Quantiser, WavInfo, WriteError};

/// Frames read and processed at once while rendering.
pub const RENDER_BLOCK: usize = 4096;

/// Switches the slots of `chain` that process audio to offline rendering, recording the
/// latency each reports in that mode. Pending parameter changes reach them with the first
/// block rendered. Waits for the UI thread.
pub fn offline(ui: &dyn UiThread, chain: &Chain) -> Result<(), String> {
    reconfigure(ui, chain, true)
}

/// Switches the slots of `chain` that process audio back to real-time playback. Waits for the
/// UI thread.
pub fn realtime(ui: &dyn UiThread, chain: &Chain) -> Result<(), String> {
    reconfigure(ui, chain, false)
}

fn reconfigure(ui: &dyn UiThread, chain: &Chain, offline: bool) -> Result<(), String> {
    let slots: Vec<Arc<Slot>> = chain.processed().cloned().collect();
    main_thread::run(ui, move || {
        // Every slot is switched even if one fails, so none is left in the wrong mode.
        let mut result = Ok(());
        for s in &slots {
            let instance = s.instance();
            let Some(config) = instance.config() else {
                result = Err(format!("“{}” isn't ready to process audio", s.name()));
                continue;
            };
            let config = Config {
                offline,
                max_block: if offline { RENDER_BLOCK } else { LIVE_BLOCK },
                ..config
            };
            match instance.configure(config) {
                Ok(latency) => s.set_latency(latency),
                Err(e) => result = Err(format!("“{}” could not render: {e}", s.name())),
            }
        }
        result
    })
    .map_err(|e| e.to_string())?
}

/// What a render wrote: the statistics and peaks of the saved audio, and how many samples
/// were clamped to fit an integer format.
pub struct Rendered {
    pub whole: Analysis,
    pub pyramid: PeakPyramid,
    pub clamped: u64,
}

/// `audio` run through a chain, as the writer saves it.
pub struct Render<'a> {
    audio: &'a Audio,
    chain: Arc<LiveChain>,
    cancel: &'a AtomicBool,
    rendered: RefCell<Option<Rendered>>,
}

impl<'a> Render<'a> {
    /// Renders `audio` through `chain`, whose slots must already be [`offline`]. Writing stops
    /// with an error once `cancel` is set.
    pub fn new(audio: &'a Audio, chain: Arc<LiveChain>, cancel: &'a AtomicBool) -> Render<'a> {
        Render {
            audio,
            chain,
            cancel,
            rendered: RefCell::new(None),
        }
    }

    /// The analysis of what was written, once the writer has had every frame.
    pub fn finish(self) -> Option<Rendered> {
        self.rendered.into_inner()
    }
}

impl NativeFrames for Render<'_> {
    fn info(&self) -> &WavInfo {
        self.audio.info()
    }

    fn fmt_bytes(&self) -> &[u8] {
        self.audio.source().fmt_bytes()
    }

    fn emit(
        &self,
        sink: &mut dyn FnMut(Frames<'_>) -> Result<(), WriteError>,
    ) -> Result<(), WriteError> {
        let info = self.audio.info();
        let (frames, ch) = (info.frames, info.channels as usize);
        let quantiser = Quantiser::new(info);
        let mut whole = Accumulator::new(info, 0, frames);
        let mut pyramid = PyramidBuilder::new(ch, frames);
        let mut clamped = 0;
        let (mut native, mut normal, mut full_scale) = (Vec::new(), Vec::new(), Vec::new());
        // Quantises processed samples, analyses them as they will read back, and writes them.
        let mut write = |processed: &[f32]| {
            native.clear();
            normal.clear();
            full_scale.clear();
            for &x in processed {
                let (v, was_clamped) = quantiser.quantise(x);
                let (n, fs) = quantiser.normalise(v);
                clamped += was_clamped as u64;
                native.push(v);
                normal.push(n);
                full_scale.push(fs);
            }
            whole.push(&normal, &full_scale);
            pyramid.push(&normal);
            sink(Frames::Native(&native))
        };

        let mut live = Live::with_block(self.chain.clone(), ch, RENDER_BLOCK);
        let (mut input, mut processed) = (Vec::new(), Vec::new());
        let mut pos = 0;
        while pos < frames {
            if self.cancel.load(Ordering::Relaxed) {
                return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled").into());
            }
            let n = (frames - pos).min(RENDER_BLOCK as u64) as usize;
            self.audio.read(pos, n, &mut input);
            processed.clear();
            live.process(&input, pos, &mut processed);
            write(&processed)?;
            pos += n as u64;
        }
        // The last `latency` frames are still in the chain.
        processed.clear();
        live.drain(frames, &mut processed);
        write(&processed)?;

        *self.rendered.borrow_mut() = Some(Rendered {
            whole: whole.finish(),
            pyramid: pyramid.finish(),
            clamped,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::live::Stage;
    use crate::fx::live::tests::Delay;
    use crate::wav::synth::{Encoding, WavBuilder};
    use crate::wav::{SampleSource, write_wav};

    fn render(audio: &Audio, stages: Vec<Arc<dyn Stage>>) -> (Vec<u8>, Rendered) {
        let cancel = AtomicBool::new(false);
        let render = Render::new(audio, Arc::new(LiveChain::new(stages)), &cancel);
        let mut out = Vec::new();
        write_wav(&render, &mut out).unwrap();
        (out, render.finish().unwrap())
    }

    #[test]
    fn latency_is_removed_and_the_length_kept() {
        let x: Vec<f64> = (0..10_000)
            .map(|i| ((i % 200) as f64 - 100.0) / 400.0)
            .collect();
        let audio =
            Audio::from_bytes(WavBuilder::new(Encoding::Pcm, 16, 2, 8_000).build(&x)).unwrap();
        let (out, rendered) = render(&audio, vec![Delay::new(1_000, 2.0, 2)]);
        let saved = SampleSource::from_bytes(out).unwrap();
        assert_eq!(saved.frames(), 5_000);
        let (mut got, mut want) = (Vec::new(), Vec::new());
        saved.read_native(0, 5_000, &mut got);
        audio.render_native(0, 5_000, &mut want);
        assert!(got.iter().zip(&want).all(|(g, w)| *g == w * 2.0));
        assert_eq!(rendered.clamped, 0);
        assert_eq!(rendered.whole.stats.end, 5_000);
    }

    #[test]
    fn integers_are_clamped_and_counted_and_floats_are_not() {
        let x = [0.75, -0.75, 0.25, 0.75];
        let pcm =
            Audio::from_bytes(WavBuilder::new(Encoding::Pcm, 16, 1, 8_000).build(&x)).unwrap();
        let (out, rendered) = render(&pcm, vec![Delay::new(0, 2.0, 1)]);
        let mut got = Vec::new();
        SampleSource::from_bytes(out)
            .unwrap()
            .read_native(0, 4, &mut got);
        assert_eq!(got, [32_767.0, -32_768.0, 16_384.0, 32_767.0]);
        assert_eq!(rendered.clamped, 3);
        // Analysed as written: three samples at full scale, in two runs too short to count.
        assert_eq!(rendered.whole.stats.channels[0].levels.peak_linear, 1.0);

        let float =
            Audio::from_bytes(WavBuilder::new(Encoding::Float, 32, 1, 8_000).build(&x)).unwrap();
        let (out, rendered) = render(&float, vec![Delay::new(0, 2.0, 1)]);
        let mut got = Vec::new();
        SampleSource::from_bytes(out)
            .unwrap()
            .read_native(0, 4, &mut got);
        assert_eq!(got, [1.5, -1.5, 0.5, 1.5]);
        assert_eq!(rendered.clamped, 0);
    }

    #[test]
    fn a_cancelled_render_stops() {
        let audio = Audio::from_bytes(
            WavBuilder::new(Encoding::Pcm, 16, 1, 8_000).build(&vec![0.1; 50_000]),
        )
        .unwrap();
        let cancel = AtomicBool::new(true);
        let stages: Vec<Arc<dyn Stage>> = vec![Delay::new(0, 1.0, 1)];
        let render = Render::new(&audio, Arc::new(LiveChain::new(stages)), &cancel);
        assert!(write_wav(&render, Vec::new()).is_err());
        assert!(render.finish().is_none());
    }
}
