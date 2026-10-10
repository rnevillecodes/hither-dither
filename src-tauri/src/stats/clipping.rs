//! Detection of clipped runs: 3 or more consecutive samples at or beyond full scale.

use serde::Serialize;

pub const MIN_RUN: u64 = 3;
/// Positions kept per channel for drawing markers; the count is always exact.
pub const MAX_STORED_RUNS: usize = 10_000;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct ClipRun {
    pub start: u64,
    pub len: u64,
}

#[derive(Debug, Clone, Default)]
pub struct ClipDetector {
    run_start: u64,
    run_len: u64,
    pub count: u64,
    pub runs: Vec<ClipRun>,
}

impl ClipDetector {
    /// Feed the full-scale flag of the sample at absolute frame `frame`.
    #[inline(always)]
    pub fn add(&mut self, frame: u64, full_scale: bool) {
        if full_scale {
            if self.run_len == 0 {
                self.run_start = frame;
            }
            self.run_len += 1;
        } else if self.run_len > 0 {
            self.end_run();
        }
    }

    pub fn finish(&mut self) {
        if self.run_len > 0 {
            self.end_run();
        }
    }

    fn end_run(&mut self) {
        if self.run_len >= MIN_RUN {
            self.count += 1;
            if self.runs.len() < MAX_STORED_RUNS {
                self.runs.push(ClipRun {
                    start: self.run_start,
                    len: self.run_len,
                });
            }
        }
        self.run_len = 0;
    }
}
