//! Peak, RMS, DC offset and crest factor.

use serde::Serialize;

#[derive(Debug, Clone, Default)]
pub struct LevelAccumulator {
    peak: f32,
    sum: f64,
    sum_sq: f64,
    n: u64,
}

/// Level statistics for one channel. `None` dB values mean silence (-inf dBFS).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Levels {
    pub peak_linear: f64,
    pub peak_dbfs: Option<f64>,
    pub rms_dbfs: Option<f64>,
    pub dc_offset_percent: f64,
    /// Peak-to-RMS ratio in dB; `None` for silence.
    pub crest_db: Option<f64>,
}

impl LevelAccumulator {
    #[inline(always)]
    pub fn add(&mut self, v: f32) {
        self.peak = self.peak.max(v.abs());
        self.sum += v as f64;
        self.sum_sq += v as f64 * v as f64;
        self.n += 1;
    }

    pub fn finish(&self) -> Levels {
        let n = self.n.max(1) as f64;
        let peak = self.peak as f64;
        let rms = (self.sum_sq / n).sqrt();
        let db = |x: f64| (x > 0.0).then(|| 20.0 * x.log10());
        let (peak_dbfs, rms_dbfs) = (db(peak), db(rms));
        Levels {
            peak_linear: peak,
            peak_dbfs,
            rms_dbfs,
            dc_offset_percent: self.sum / n * 100.0,
            crest_db: peak_dbfs.zip(rms_dbfs).map(|(p, r)| p - r),
        }
    }
}
