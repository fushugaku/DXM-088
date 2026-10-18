use crate::params::TRACKS;
pub const PATTERNS: usize = 128;
pub const STEPS: usize = 64;

#[derive(Clone, Copy, Debug)]
pub struct Lock {
    pub param: u8,
    pub value: f32,
}
impl Default for Lock {
    fn default() -> Self {
        Self {
            param: 255,
            value: 0.,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Step {
    pub velocity: f32,
    pub probability: f32,
    pub note: i8,
    pub ratchet: u8,
    pub locks: [Lock; 2],
    pub micro: f32,
    pub condition: u8,
    pub every: u8,
    pub phase: u8,
}
impl Default for Step {
    fn default() -> Self {
        Self {
            velocity: 0.,
            probability: 1.,
            note: 0,
            ratchet: 1,
            locks: [Lock::default(); 2],
            micro: 0.,
            condition: 0,
            every: 2,
            phase: 0,
        }
    }
}
#[derive(Clone, Copy)]
pub struct Track {
    pub steps: [Step; STEPS],
    pub length: usize,
    pub division: u8,
    pub delay: f32,
    pub groove: u8,
    pub groove_amount: f32,
}
impl Default for Track {
    fn default() -> Self {
        Self {
            steps: [Step::default(); STEPS],
            length: 16,
            division: 4,
            delay: 0.,
            groove: 0,
            groove_amount: 0.,
        }
    }
}
#[derive(Clone, Copy, Default)]
pub struct Pattern {
    pub tracks: [Track; TRACKS],
}

/// Allocate the bank on the heap: the full MIDI bank exceeds common host stacks.
pub fn blank_patterns() -> Box<[Pattern; PATTERNS]> {
    vec![Pattern::default(); PATTERNS]
        .into_boxed_slice()
        .try_into()
        .unwrap_or_else(|_| unreachable!())
}

pub fn demo_patterns() -> Box<[Pattern; PATTERNS]> {
    let mut patterns = blank_patterns();
    for (p, pattern) in patterns.iter_mut().enumerate() {
        let hits: [&[usize]; TRACKS] = match p % 4 {
            0 => [
                &[0, 4, 8, 12],
                &[6, 14],
                &[3, 11],
                &[4, 12],
                &[12],
                &[0, 2, 4, 6, 8, 10, 12, 14],
                &[7, 15],
            ],
            1 => [
                &[0, 3, 8, 10, 14],
                &[5, 13],
                &[2, 7, 11],
                &[4, 12],
                &[6, 14],
                &[0, 2, 3, 6, 8, 10, 11, 14],
                &[7, 15],
            ],
            2 => [
                &[0, 6, 8, 14],
                &[3, 10],
                &[7, 15],
                &[4, 12],
                &[4, 12],
                &[0, 2, 4, 6, 8, 10, 12, 14],
                &[3, 11],
            ],
            _ => [
                &[0, 4, 8, 11],
                &[2, 14],
                &[6, 10, 15],
                &[4, 12, 15],
                &[7],
                &[0, 1, 4, 6, 8, 9, 12, 14],
                &[3, 11, 15],
            ],
        };
        for (t, positions) in hits.iter().enumerate() {
            for &s in *positions {
                let step = &mut pattern.tracks[t].steps[s];
                step.velocity = if s % 4 == 0 {
                    0.95
                } else {
                    0.67 + (s % 3) as f32 * 0.08
                };
                if p % 8 >= 4 && t > 0 && s % 4 != 0 {
                    step.probability = 0.72;
                }
            }
        }
    }
    patterns
}

pub fn euclidean(track: &mut Track, pulses: usize, length: usize, rotation: usize) {
    let n = length.clamp(1, STEPS);
    let k = pulses.min(n);
    track.length = n;
    for i in 0..STEPS {
        let s = Step {
            velocity: if i < n && (((i + n - rotation % n) % n) * k) % n < k {
                0.85
            } else {
                0.
            },
            ..Step::default()
        };
        track.steps[i] = s;
    }
}

#[derive(Clone, Copy, Default)]
pub struct Ratchet {
    pub remaining: u8,
    pub countdown: f64,
    pub interval: f64,
    pub velocity: f32,
    pub note: i8,
}
