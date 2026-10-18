//! Master effect implementations from 0x080130c8 / 0x08013e34 / 0x08012380.
use super::{math, oscillator::Oscillator, sat16, wrap16, OnePole, PCM_SCALE};
const ROM: &[u8; 2072] = include_bytes!("../data/drive-rom.bin");
fn tube(index: usize) -> f64 {
    let p = 1040 + index * 8;
    f64::from_le_bytes(ROM[p..p + 8].try_into().unwrap())
}
fn clip(index: usize) -> f32 {
    let p = 8 + index * 4;
    f32::from_le_bytes(ROM[p..p + 4].try_into().unwrap())
}

/// Shared wet-minus-dry mixer; original effects saturate the difference itself.
#[inline]
pub fn blend(dry: i16, wet: i16, mix: f32) -> i16 {
    let difference = sat16(((wet as i32 - dry as i32) as f32 * mix) as i32);
    sat16(dry as i32 + difference as i32)
}

#[derive(Clone, Copy, Debug)]
pub struct Drive {
    pub kind: u8,
    pub amount: f32,
    pub shape: f32,
    pub tone: f32,
    pub level: f32,
    pub smooth: [f64; 2],
    pub fast_lp: [f64; 2],
    pub fast_hp: [f64; 2],
    pub slow_lp: [f64; 2],
    pub slow_hp: [f64; 2],
    pub slew: [f64; 2],
    pub feedback: [f64; 2],
}
impl Default for Drive {
    fn default() -> Self {
        Self {
            kind: 0,
            amount: 0.,
            shape: 0.,
            tone: 1.,
            level: 1.,
            smooth: [0.; 2],
            fast_lp: [0.; 2],
            fast_hp: [0.; 2],
            slow_lp: [0.; 2],
            slow_hp: [0.; 2],
            slew: [0.; 2],
            feedback: [0.; 2],
        }
    }
}
impl Drive {
    #[inline]
    pub fn tick(&mut self, input: [i16; 2], mix: f32) -> [i16; 2] {
        let coefficient = (self.tone as f64).mul_add(0.9241070173442663, 0.06537948995454451);
        let mut out = [0; 2];
        for c in 0..2 {
            let wet = match self.kind {
                0 => {
                    let gain = self.amount.mul_add(4., 1.);
                    let scaled = (input[c] as f32 * gain) as i32;
                    let index = ((sat16(scaled) as i32 >> 9) + 64) as usize;
                    let fraction = (scaled & 511) as f32 * 0.001953125;
                    let a = tube(index);
                    let b = tube(index + 1);
                    let delta = (b - a).mul_add(fraction as f64, a - self.smooth[c]);
                    self.smooth[c] = delta.mul_add(coefficient, self.smooth[c]);
                    let boost = ((self.shape as f64).mul_add(1.1, 1.)) as f32;
                    let amplitude = (self.level as f64 * 32768.) * boost as f64;
                    sat16((self.smooth[c] * amplitude) as i32)
                }
                1 => {
                    let gain = self.amount.mul_add(4., 1.) * (1. / 32768.);
                    let x = (input[c] as f32).mul_add(gain, self.shape);
                    self.smooth[c] =
                        (x as f64 - self.smooth[c]).mul_add(coefficient, self.smooth[c]);
                    let mut x = self.smooth[c] as f32;
                    if !(-1. ..=1.).contains(&x) {
                        x = ((((x - 1.) as f64 % 4.).abs() - 2.).abs() - 1.) as f32;
                    }
                    sat16(((x * self.level) * 32768.) as i32)
                }
                _ => {
                    let feedback = self.shape as f64 * 0.125;
                    let x = (input[c] as f64).mul_add(1. / 32768., self.feedback[c] * feedback);
                    self.fast_lp[c] =
                        (x - self.fast_lp[c]).mul_add(0.6411664192013119, self.fast_lp[c]);
                    self.fast_hp[c] = ((x - self.fast_hp[c]) - self.fast_lp[c])
                        .mul_add(0.8974646758371088, self.fast_hp[c]);
                    self.slow_lp[c] = (self.fast_hp[c] - self.slow_lp[c])
                        .mul_add(0.008504435618457151, self.slow_lp[c]);
                    self.slow_hp[c] = ((self.fast_hp[c] - self.slow_hp[c]) - self.slow_lp[c])
                        .mul_add(0.06869958357054795, self.slow_hp[c]);
                    let gain = self.amount.mul_add(2304., 1.) as f64;
                    let mut x = self.slow_hp[c] * gain;
                    let delta = x - self.slew[c];
                    if delta.abs() > 6.796556411418215 {
                        x = self.slew[c] + 6.796556411418215_f64.copysign(delta);
                    }
                    self.slew[c] = x.clamp(-4.2, 4.2);
                    let y = if x > 4.2 {
                        0.6470419764518738_f32
                    } else if x < -4.2 {
                        -0.6474789977073669_f32
                    } else {
                        let p = ((x as f32).clamp(-1., 1.) + 1.) * 128.;
                        let index = p as usize;
                        let fraction = (p - index as f32) as f64;
                        let base = (((1. - p) + index as f32) * clip(index)) as f64;
                        (clip(index + 1) as f64).mul_add(fraction, base) as f32
                    };
                    self.smooth[c] =
                        (y as f64 - self.smooth[c]).mul_add(coefficient, self.smooth[c]);
                    self.feedback[c] = self.smooth[c] * self.level as f64;
                    sat16((self.feedback[c] * 32768.) as i32)
                }
            };
            out[c] = blend(input[c], wet, mix);
        }
        out
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Ring {
    pub oscillator: Oscillator,
    pub random: u32,
}
impl Default for Ring {
    fn default() -> Self {
        Self {
            oscillator: Oscillator::default(),
            random: 0x7e51b49a,
        }
    }
}
impl Ring {
    pub fn configure(&mut self, frequency: f32, sample_rate: f32) {
        self.oscillator.tune(frequency, sample_rate);
    }
    #[inline]
    pub fn tick(&mut self, input: [i16; 2], mix: f32) -> [i16; 2] {
        let random = &mut self.random;
        let carrier = self.oscillator.tick(1., 0.3, None, || {
            *random ^= *random << 13;
            *random ^= *random >> 17;
            *random ^= *random << 5;
            *random as i16
        }) as i32;
        input.map(|dry| blend(dry, ((dry as i32 * carrier) >> 16) as i16, mix))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Compressor {
    pub threshold: f32,
    pub attack: f32,
    pub release: f32,
    pub ratio: f32,
    pub makeup: f32,
    pub envelope: f64,
}
impl Default for Compressor {
    fn default() -> Self {
        Self {
            threshold: -24.,
            attack: 0.999,
            release: 0.9999,
            ratio: 0.25,
            makeup: 1.,
            envelope: 0.,
        }
    }
}
impl Compressor {
    pub fn configure(
        &mut self,
        attack: f32,
        release: f32,
        threshold: f32,
        makeup: f32,
        ratio: f32,
        sample_rate: f32,
    ) {
        let attack_ms = attack.mul_add(100., 0.03);
        let release_ms = release.mul_add(175., 25.);
        self.attack = (-1000_f64 / (attack_ms * sample_rate) as f64).exp() as f32;
        self.release = (-1000_f64 / (release_ms * sample_rate) as f64).exp() as f32;
        self.threshold = (threshold - 1.) * 96.;
        self.makeup = makeup.mul_add(2., 1.);
        self.ratio = ratio;
    }
    #[inline]
    pub fn tick(&mut self, input: [i16; 2], mix: f32) -> [i16; 2] {
        // The hardware detects the signed stereo mean, including cancellation.
        let level =
            ((input[0] as f32 * (1. / 32768.) + input[1] as f32 * (1. / 32768.)) * 0.5).abs();
        let logarithm = math::log((level as f64 + 1e-25) as f32);
        let over = (logarithm as f64)
            .mul_add(8.685889638065037, -(self.threshold as f64))
            .max(0.);
        let coefficient = if over <= self.envelope {
            self.release
        } else {
            self.attack
        };
        self.envelope = (self.envelope - over).mul_add(coefficient as f64, over);
        let multiplier = (self.ratio as f64 - 1.) * 0.11512925464970228;
        let gain = math::exp((self.envelope * multiplier) as f32);
        input.map(|dry| {
            blend(
                dry,
                sat16(((dry as f64 * self.makeup as f64) * gain as f64) as i32),
                mix,
            )
        })
    }
}

/// 0x080113ec: per-voice rational shaper, distinct from the master drive.
#[derive(Clone, Copy, Debug, Default)]
pub struct Shaper {
    pub amount: f32,
    pub smooth: OnePole,
}
impl Shaper {
    /// Unsmoothed variant used by snare/cymbal/HH at ITCM 0x1f70.
    #[inline]
    pub fn tick_static(&self, sample: i16) -> i16 {
        let x = sample as f32 * PCM_SCALE;
        wrap16((((self.amount + 1.) * x) / x.abs().mul_add(self.amount, 1.)) * 32767.)
    }
    #[inline]
    pub fn tick(&mut self, sample: i16) -> i16 {
        let k = self.smooth.tick(self.amount as f64);
        let x = sample as f32 * PCM_SCALE;
        wrap16((((k + 1.) * x) / x.abs().mul_add(k, 1.)) * 32767.)
    }
}
