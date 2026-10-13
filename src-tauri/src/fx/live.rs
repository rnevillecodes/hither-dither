//! Realtime processing of a chain for the playback feeder.
//!
//! A [`LiveChain`] is the immutable snapshot playback processes: the stages that process
//! audio, in order, and their total latency. The feeder owns a [`Live`] for it, holding the
//! planar buffers and how much of the chain's output still has to be dropped to remove its
//! latency.

use std::sync::Arc;

use super::chain::{Chain, LIVE_BLOCK, Slot};
use crate::plugins::instance::Transport;

/// One processing step of a live chain. Implemented by [`Slot`]; tests add their own.
pub trait Stage: Send + Sync {
    /// Processing delay in frames.
    fn latency(&self) -> u32;

    /// Processes `frames` frames from `input` into `output`, one slice per channel. Returns
    /// false, leaving `output` untouched, if it couldn't.
    fn process(
        &self,
        input: &[&[f32]],
        output: &mut [&mut [f32]],
        frames: usize,
        transport: Transport,
    ) -> bool;
}

impl Stage for Slot {
    fn latency(&self) -> u32 {
        Slot::latency(self)
    }

    fn process(
        &self,
        input: &[&[f32]],
        output: &mut [&mut [f32]],
        frames: usize,
        transport: Transport,
    ) -> bool {
        self.instance().process(input, output, frames, transport)
    }
}

/// The stages playback runs the audio through, in order, with their total latency.
pub struct LiveChain {
    stages: Vec<Arc<dyn Stage>>,
    latency: u32,
}

impl LiveChain {
    pub fn new(stages: Vec<Arc<dyn Stage>>) -> LiveChain {
        let latency = stages.iter().map(|s| s.latency()).sum();
        LiveChain { stages, latency }
    }

    /// What playback of `chain` processes, or `None` when no slot processes audio.
    pub fn of(chain: &Chain) -> Option<Arc<LiveChain>> {
        let stages: Vec<Arc<dyn Stage>> = chain
            .processed()
            .map(|s| s.clone() as Arc<dyn Stage>)
            .collect();
        (!stages.is_empty()).then(|| Arc::new(LiveChain::new(stages)))
    }

    /// Total latency in frames, as it was when the snapshot was taken.
    pub fn latency(&self) -> u32 {
        self.latency
    }

    /// Whether `a` and `b` process the same way: the same stages, in order, with the same
    /// latency. Switching between them needn't flush playback.
    pub fn same(a: Option<&Arc<LiveChain>>, b: Option<&Arc<LiveChain>>) -> bool {
        match (a, b) {
            (None, None) => true,
            (Some(a), Some(b)) => {
                a.latency == b.latency
                    && a.stages.len() == b.stages.len()
                    && a.stages
                        .iter()
                        .zip(&b.stages)
                        .all(|(x, y)| std::ptr::addr_eq(Arc::as_ptr(x), Arc::as_ptr(y)))
            }
            _ => false,
        }
    }
}

/// The feeder's side of a live chain: buffers, and the latency still to remove.
pub struct Live {
    chain: Arc<LiveChain>,
    channels: usize,
    /// Most frames passed to a stage at once.
    block: usize,
    /// Planar input and output of a stage; swapped after each stage that processed.
    a: Vec<Vec<f32>>,
    b: Vec<Vec<f32>>,
    /// Processed frames still to drop before the output lines up with the input.
    discard: usize,
}

impl Live {
    /// Prepares to process `channels`-channel audio through `chain`, from a fresh start.
    pub fn new(chain: Arc<LiveChain>, channels: usize) -> Live {
        Live::with_block(chain, channels, LIVE_BLOCK)
    }

    /// Like [`Live::new`], passing the stages at most `block` frames at once.
    pub fn with_block(chain: Arc<LiveChain>, channels: usize, block: usize) -> Live {
        let discard = chain.latency as usize;
        Live {
            chain,
            channels,
            block,
            a: vec![vec![0.0; block]; channels],
            b: vec![vec![0.0; block]; channels],
            discard,
        }
    }

    pub fn chain(&self) -> &Arc<LiveChain> {
        &self.chain
    }

    /// The input jumps (play start, or a flush): drop the next `latency` frames of output,
    /// which belong to the audio before the jump.
    pub fn restart(&mut self) {
        self.discard = self.chain.latency as usize;
    }

    /// Processes interleaved `input`, whose first frame is file frame `position`, and appends
    /// the output that lines up with the input fed so far to `out`.
    pub fn process(&mut self, input: &[f32], position: u64, out: &mut Vec<f32>) {
        let ch = self.channels;
        for (k, block) in input.chunks(self.block * ch).enumerate() {
            let n = block.len() / ch;
            for (c, plane) in self.a.iter_mut().enumerate() {
                for (dst, frame) in plane.iter_mut().zip(block.chunks_exact(ch)) {
                    *dst = frame[c];
                }
            }
            self.run(n, position + (k * self.block) as u64, out);
        }
    }

    /// Feeds `latency` frames of silence, from file frame `position`, and appends the end of
    /// the processed audio to `out`. After this, as many frames have come out as went in.
    pub fn drain(&mut self, position: u64, out: &mut Vec<f32>) {
        let mut left = self.chain.latency as usize;
        let mut at = position;
        while left > 0 {
            let n = left.min(self.block);
            for plane in &mut self.a {
                plane[..n].fill(0.0);
            }
            self.run(n, at, out);
            left -= n;
            at += n as u64;
        }
    }

    /// Runs the first `n` frames of `a` through every stage and appends the result,
    /// interleaved and less any frames still to discard, to `out`.
    fn run(&mut self, n: usize, position: u64, out: &mut Vec<f32>) {
        let transport = Transport {
            playing: true,
            position: position as i64,
        };
        for stage in &self.chain.stages {
            let input: Vec<&[f32]> = self.a.iter().map(|c| &c[..n]).collect();
            let mut output: Vec<&mut [f32]> = self.b.iter_mut().map(|c| &mut c[..n]).collect();
            // A stage that fails passes its input on.
            if stage.process(&input, &mut output, n, transport) {
                std::mem::swap(&mut self.a, &mut self.b);
            }
        }
        let skip = self.discard.min(n);
        self.discard -= skip;
        out.reserve((n - skip) * self.channels);
        for i in skip..n {
            out.extend(self.a.iter().map(|c| c[i]));
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Scales by `gain` and delays by `latency` frames, like HD Test Gain.
    pub struct Delay {
        pub latency: usize,
        pub gain: f32,
        lines: Mutex<Vec<std::collections::VecDeque<f32>>>,
    }

    impl Delay {
        pub fn new(latency: usize, gain: f32, channels: usize) -> Arc<Delay> {
            let line = std::iter::repeat_n(0.0, latency).collect();
            Arc::new(Delay {
                latency,
                gain,
                lines: Mutex::new(vec![line; channels]),
            })
        }
    }

    impl Stage for Delay {
        fn latency(&self) -> u32 {
            self.latency as u32
        }

        fn process(
            &self,
            input: &[&[f32]],
            output: &mut [&mut [f32]],
            frames: usize,
            _: Transport,
        ) -> bool {
            let mut lines = self.lines.lock().unwrap();
            for ((line, i), o) in lines.iter_mut().zip(input).zip(output.iter_mut()) {
                for n in 0..frames {
                    line.push_back(i[n] * self.gain);
                    o[n] = line.pop_front().unwrap();
                }
            }
            true
        }
    }

    fn interleaved(frames: usize, channels: usize) -> Vec<f32> {
        (0..frames * channels).map(|i| (i + 1) as f32).collect()
    }

    #[test]
    fn output_lines_up_with_input_and_drains_to_the_same_length() {
        let chain = Arc::new(LiveChain::new(vec![
            Delay::new(100, 2.0, 2) as Arc<dyn Stage>,
            Delay::new(1500, 0.5, 2),
        ]));
        assert_eq!(chain.latency(), 1600);
        let mut live = Live::new(chain, 2);
        let input = interleaved(3000, 2);
        let mut out = Vec::new();
        for (k, piece) in input.chunks(700 * 2).enumerate() {
            live.process(piece, k as u64 * 700, &mut out);
        }
        live.drain(3000, &mut out);
        assert_eq!(out, input);
    }

    #[test]
    fn a_range_shorter_than_the_latency_still_comes_out_whole() {
        let chain = Arc::new(LiveChain::new(vec![
            Delay::new(64, 1.0, 1) as Arc<dyn Stage>
        ]));
        let mut live = Live::new(chain, 1);
        let mut out = Vec::new();
        live.process(&[1.0, 2.0, 3.0], 0, &mut out);
        assert!(out.is_empty());
        live.drain(3, &mut out);
        assert_eq!(out, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn restart_drops_the_audio_from_before_the_jump() {
        let chain = Arc::new(LiveChain::new(vec![
            Delay::new(10, 1.0, 1) as Arc<dyn Stage>
        ]));
        let mut live = Live::new(chain, 1);
        let mut out = Vec::new();
        live.process(&interleaved(50, 1), 0, &mut out);
        out.clear();
        live.restart();
        let jumped: Vec<f32> = (0..40).map(|i| 1000.0 + i as f32).collect();
        live.process(&jumped, 500, &mut out);
        assert_eq!(out, jumped[..30]);
    }

    #[test]
    fn chains_are_the_same_when_their_stages_are() {
        let a: Arc<dyn Stage> = Delay::new(1, 1.0, 1);
        let b: Arc<dyn Stage> = Delay::new(1, 1.0, 1);
        let ab = Arc::new(LiveChain::new(vec![a.clone(), b.clone()]));
        let ab2 = Arc::new(LiveChain::new(vec![a.clone(), b.clone()]));
        let ba = Arc::new(LiveChain::new(vec![b, a]));
        assert!(LiveChain::same(Some(&ab), Some(&ab2)));
        assert!(!LiveChain::same(Some(&ab), Some(&ba)));
        assert!(!LiveChain::same(Some(&ab), None));
        assert!(LiveChain::same(None, None));
    }
}
