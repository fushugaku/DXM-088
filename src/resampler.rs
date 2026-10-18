//! The configured I2S clock is 44,140.625 Hz. Resampling follows master FX.
//! A causal 32-tap Blackman-windowed sinc keeps host AudioContext rates out of DSP.
use crate::firmware::CLOCK;
use crate::OUTPUT_CHANNELS;
const TAPS: usize = 32;
const PHASES: usize = 256;
pub struct Resampler {
    pub phase: f64,
    pub step: f64,
    history: [[f32; OUTPUT_CHANNELS]; TAPS],
    head: usize,
    coefficients: Box<[[f32; TAPS]; PHASES + 1]>,
    bypass: bool,
}
impl Resampler {
    pub fn new(rate: f32) -> Self {
        let cutoff = (rate as f64 / CLOCK).min(1.) * 0.96;
        let coefficients = Box::new(std::array::from_fn(|p| {
            let fraction = p as f64 / PHASES as f64;
            let mut taps = std::array::from_fn(|i| {
                let x = i as f64 + fraction - 15.;
                let sinc = if x.abs() < 1e-12 {
                    cutoff
                } else {
                    (std::f64::consts::PI * x * cutoff).sin() / (std::f64::consts::PI * x)
                };
                let window = 0.42
                    + 0.5 * (std::f64::consts::PI * x / 16.).cos()
                    + 0.08 * (std::f64::consts::PI * x / 8.).cos();
                (sinc * window) as f32
            });
            let sum = taps.iter().map(|&x| x as f64).sum::<f64>();
            for x in &mut taps {
                *x = (*x as f64 / sum) as f32;
            }
            taps
        }));
        Self {
            phase: 0.,
            step: CLOCK / rate as f64,
            history: [[0.; OUTPUT_CHANNELS]; TAPS],
            head: 0,
            coefficients,
            bypass: rate as f64 == CLOCK,
        }
    }
    pub fn bypass(&self) -> bool {
        self.bypass
    }
    pub fn clear(&mut self) {
        self.phase = 0.;
        self.history.fill([0.; OUTPUT_CHANNELS]);
        self.head = 0;
    }
    pub fn push(&mut self, sample: [f32; OUTPUT_CHANNELS]) {
        self.head = (self.head + 1) % TAPS;
        self.history[self.head] = sample;
    }
    pub fn output(&self, channels: usize) -> [f32; OUTPUT_CHANNELS] {
        let phase = self.phase * PHASES as f64;
        let index = (phase as usize).min(PHASES - 1);
        let fraction = (phase - index as f64) as f32;
        let mut out = [0.; OUTPUT_CHANNELS];
        for i in 0..TAPS {
            let weight = (self.coefficients[index + 1][i] - self.coefficients[index][i])
                .mul_add(fraction, self.coefficients[index][i]);
            let value = self.history[(self.head + TAPS - i) % TAPS];
            for c in 0..channels {
                out[c] += value[c] * weight;
            }
        }
        out.map(|x| x.clamp(-1., 1.))
    }
}
