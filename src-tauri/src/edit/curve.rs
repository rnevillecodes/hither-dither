//! Fade curve shape: a power curve with one bend value. The same formula is in
//! `src/lib/curve.ts`; both test suites check the same table of values.

use serde::{Deserialize, Serialize};

/// Whether a fade rises from silence (`In`) or falls to it (`Out`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Dir {
    In,
    Out,
}

/// Bend in [-1, 1]: 0 is linear, positive bows the fade line upward (louder through the
/// fade), negative bows it downward.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Curve {
    pub bend: f64,
}

impl Curve {
    pub const LINEAR: Curve = Curve { bend: 0.0 };

    pub fn new(bend: f64) -> Self {
        let bend = if bend.is_finite() {
            bend.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        Self { bend }
    }

    /// Exponent of the power curve: 4^-bend, from 1/4 (bend 1) to 4 (bend -1).
    fn power(self) -> f64 {
        4f64.powf(-self.bend.clamp(-1.0, 1.0))
    }

    /// Gain at position `t` (0 at the fade's start, 1 at its end).
    pub fn gain(self, dir: Dir, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        let x = match dir {
            Dir::In => t,
            Dir::Out => 1.0 - t,
        };
        if self.bend == 0.0 {
            x
        } else {
            x.powf(self.power())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fade-in gain at t = 0, 1/4, 1/2, 3/4, 1 for each bend. Shared with `curve.test.ts`.
    // Literal values, kept identical to the TypeScript copy.
    #[allow(clippy::approx_constant)]
    const TABLE: [(f64, [f64; 5]); 5] = [
        (-1.0, [0.0, 0.00390625, 0.0625, 0.31640625, 1.0]),
        (-0.5, [0.0, 0.0625, 0.25, 0.5625, 1.0]),
        (0.0, [0.0, 0.25, 0.5, 0.75, 1.0]),
        (0.5, [0.0, 0.5, 0.7071067811865476, 0.8660254037844386, 1.0]),
        (
            1.0,
            [
                0.0,
                0.7071067811865476,
                0.8408964152537145,
                0.9306048591020996,
                1.0,
            ],
        ),
    ];

    #[test]
    fn matches_the_shared_table() {
        for (bend, gains) in TABLE {
            let c = Curve::new(bend);
            for (i, want) in gains.into_iter().enumerate() {
                let t = i as f64 / 4.0;
                assert!(
                    (c.gain(Dir::In, t) - want).abs() < 1e-12,
                    "bend {bend} t {t}"
                );
                // A fade-out is the mirror image.
                assert!((c.gain(Dir::Out, 1.0 - t) - want).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn clamps_bend_and_position() {
        assert_eq!(Curve::new(5.0).bend, 1.0);
        assert_eq!(Curve::new(f64::NAN), Curve::LINEAR);
        assert_eq!(Curve::LINEAR.gain(Dir::In, -1.0), 0.0);
        assert_eq!(Curve::LINEAR.gain(Dir::In, 2.0), 1.0);
    }
}
