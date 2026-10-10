//! Mapping file channels onto device channels (playback only; analysis is unaffected).

/// How file channels are routed to the device's channels.
#[derive(Debug, Clone, PartialEq)]
pub struct ChannelMap {
    in_ch: usize,
    out_ch: usize,
    /// `gains[o * in_ch + i]`: contribution of input `i` to output `o`.
    gains: Vec<f32>,
    /// Down-mixes can exceed full scale and need the safety limiter.
    limit: bool,
}

const MINUS_3DB: f32 = std::f32::consts::FRAC_1_SQRT_2;

impl ChannelMap {
    /// `five_one` marks a 6-channel file in standard WAV 5.1 order (L R C LFE Ls Rs).
    pub fn new(in_ch: usize, out_ch: usize, five_one: bool) -> Self {
        let mut gains = vec![0.0; in_ch * out_ch];
        let mut set = |o: usize, i: usize, g: f32| gains[o * in_ch + i] = g;
        let mut limit = false;
        if in_ch == 1 {
            // Mono plays on every output (both sides of a stereo device).
            for o in 0..out_ch {
                set(o, 0, 1.0);
            }
        } else if in_ch <= out_ch {
            for i in 0..in_ch {
                set(i, i, 1.0);
            }
        } else if out_ch == 1 {
            let g = 1.0 / (in_ch as f32).sqrt();
            for i in 0..in_ch {
                set(0, i, g);
            }
            limit = true;
        } else if five_one && in_ch == 6 {
            // ITU-R BS.775 down-mix to the first two outputs; LFE is dropped.
            set(0, 0, 1.0);
            set(1, 1, 1.0);
            set(0, 2, MINUS_3DB);
            set(1, 2, MINUS_3DB);
            set(0, 4, MINUS_3DB);
            set(1, 5, MINUS_3DB);
            limit = true;
        } else {
            // Alternate extra channels between left and right, scaled by 1/√N per side.
            let per_side = [in_ch.div_ceil(2), in_ch / 2];
            for i in 0..in_ch {
                let side = i % 2;
                set(side, i, 1.0 / (per_side[side] as f32).sqrt());
            }
            limit = true;
        }
        Self {
            in_ch,
            out_ch,
            gains,
            limit,
        }
    }

    pub fn out_channels(&self) -> usize {
        self.out_ch
    }

    /// Map interleaved input frames to interleaved output frames, appending to `out`.
    pub fn apply(&self, input: &[f32], out: &mut Vec<f32>) {
        for frame in input.chunks_exact(self.in_ch) {
            for o in 0..self.out_ch {
                let row = &self.gains[o * self.in_ch..(o + 1) * self.in_ch];
                let v: f32 = row.iter().zip(frame).map(|(g, x)| g * x).sum();
                out.push(if self.limit { soft_limit(v) } else { v });
            }
        }
    }
}

/// Transparent below 0.9, then eases towards ±1.0 instead of hard clipping.
pub fn soft_limit(v: f32) -> f32 {
    const KNEE: f32 = 0.9;
    let a = v.abs();
    if a <= KNEE {
        v
    } else {
        v.signum() * (KNEE + (1.0 - KNEE) * ((a - KNEE) / (1.0 - KNEE)).tanh())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(m: &ChannelMap, input: &[f32]) -> Vec<f32> {
        let mut out = Vec::new();
        m.apply(input, &mut out);
        out
    }

    #[test]
    fn mono_plays_on_both_stereo_channels() {
        let m = ChannelMap::new(1, 2, false);
        assert_eq!(map(&m, &[0.5, -0.25]), [0.5, 0.5, -0.25, -0.25]);
    }

    #[test]
    fn stereo_maps_one_to_one_and_pads_wider_devices() {
        assert_eq!(map(&ChannelMap::new(2, 2, false), &[0.1, 0.2]), [0.1, 0.2]);
        assert_eq!(
            map(&ChannelMap::new(2, 4, false), &[0.1, 0.2]),
            [0.1, 0.2, 0.0, 0.0]
        );
    }

    #[test]
    fn five_one_downmix_drops_lfe() {
        let m = ChannelMap::new(6, 2, true);
        let out = map(&m, &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
        assert_eq!(out, [0.0, 0.0]);
        let out = map(&m, &[0.0, 0.0, 0.5, 0.0, 0.0, 0.0]);
        assert!((out[0] - 0.5 * MINUS_3DB).abs() < 1e-6 && (out[1] - out[0]).abs() < 1e-6);
    }

    #[test]
    fn generic_downmix_scales_by_inverse_sqrt_and_limits() {
        let m = ChannelMap::new(4, 2, false);
        // Two channels per side, each scaled by 1/√2.
        let out = map(&m, &[0.1, 0.0, 0.1, 0.0]);
        assert!((out[0] - 0.2 / 2f32.sqrt()).abs() < 1e-6);
        assert_eq!(out[1], 0.0);
        // Full-scale on every channel would sum to √2; the limiter keeps it below 1.0.
        let out = map(&m, &[1.0, 1.0, 1.0, 1.0]);
        assert!(out.iter().all(|v| *v <= 1.0 && *v > 0.9));
        let m = ChannelMap::new(3, 1, false);
        let out = map(&m, &[0.3, 0.3, 0.3]);
        assert!((out[0] - 0.9 / 3f32.sqrt()).abs() < 1e-6);
    }

    #[test]
    fn soft_limit_is_transparent_below_knee() {
        assert_eq!(soft_limit(0.5), 0.5);
        assert_eq!(soft_limit(-0.9), -0.9);
        assert!(soft_limit(5.0) <= 1.0 && soft_limit(-5.0) >= -1.0);
    }
}
