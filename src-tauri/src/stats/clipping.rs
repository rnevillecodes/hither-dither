//! Detection of clipped runs: 3 or more consecutive samples at or beyond full scale.

use serde::Serialize;

use crate::edit::Span;

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

/// The source's clip runs as they appear in edited audio whose output is `spans`: only
/// verbatim copies keep their runs (changed audio is never a raw full-scale copy), each cut
/// to the copy and moved to its output position; runs cut below [`MIN_RUN`] are dropped.
pub fn map_runs(runs: &[ClipRun], spans: &[Span]) -> Vec<ClipRun> {
    let mut out = Vec::new();
    for span in spans {
        let Some(from) = span.src else { continue };
        let to = from + span.len;
        let first = runs.partition_point(|r| r.start + r.len <= from);
        for r in runs[first..].iter().take_while(|r| r.start < to) {
            let (a, b) = (r.start.max(from), (r.start + r.len).min(to));
            if b - a >= MIN_RUN {
                out.push(ClipRun {
                    start: span.start + (a - from),
                    len: b - a,
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(start: u64, len: u64) -> ClipRun {
        ClipRun { start, len }
    }

    fn copy(start: u64, len: u64, src: u64) -> Span {
        Span {
            start,
            len,
            src: Some(src),
        }
    }

    fn changed(start: u64, len: u64) -> Span {
        Span {
            start,
            len,
            src: None,
        }
    }

    #[test]
    fn runs_in_changed_audio_vanish() {
        let runs = [run(10, 5), run(50, 4), run(90, 3)];
        // 40..70 silenced or faded: the run at 50 is gone, the others stay put.
        let spans = [copy(0, 40, 0), changed(40, 30), copy(70, 30, 70)];
        assert_eq!(map_runs(&runs, &spans), [run(10, 5), run(90, 3)]);
    }

    #[test]
    fn runs_after_a_delete_move_left() {
        let runs = [run(10, 5), run(150, 4)];
        // 20..120 deleted with a hard join.
        let spans = [copy(0, 20, 0), copy(20, 80, 120)];
        assert_eq!(map_runs(&runs, &spans), [run(10, 5), run(50, 4)]);
    }

    #[test]
    fn runs_cut_short_are_dropped() {
        let runs = [run(18, 6), run(40, 10)];
        // A crossfade covers 20..44: the first run keeps 2 frames (dropped), the second
        // keeps 6 frames after the crossfade.
        let spans = [copy(0, 20, 0), changed(20, 24), copy(44, 56, 44)];
        assert_eq!(map_runs(&runs, &spans), [run(44, 6)]);
    }
}
