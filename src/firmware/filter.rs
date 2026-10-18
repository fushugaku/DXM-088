//! Original nonlinear SVF / LP2 at 0x18d4, including the rational tan prewarp.
use super::{sat16, PCM_SCALE};
#[derive(Clone, Copy, Debug)]
pub struct Filter {
    pub state: [f32; 8],
}
impl Default for Filter {
    fn default() -> Self {
        Self {
            state: [0.1, 0.3, 0.8, 0., 0., 0., 0., 0.4],
        }
    }
}
#[inline]
fn nonlinear(x: f32) -> f32 {
    let half = x * 0.5;
    let s = half * half;
    (s + 105.).mul_add(s, 945.) / s.mul_add(s.mul_add(15., 420.), 945.)
}
impl Filter {
    pub fn configure(&mut self, hz: f32, resonance: f32, drive: f32, sample_rate: f32) {
        self.state[0] = (hz / sample_rate).clamp(0., 0.45);
        let f = (self.state[0] as f64 * std::f64::consts::PI) as f32;
        let f2 = f * f;
        self.state[1] = ((f2 - 15.) * (f * (1. / 3.))) / f2.mul_add(2., -5.);
        self.state[2] = if resonance > 0.9 {
            0.02
        } else {
            1. - resonance
        };
        self.state[7] = (drive * drive).mul_add(6., 0.4);
    }
    #[inline]
    pub fn tick(&mut self, sample: i16, kind: u8) -> i16 {
        if kind == 0 || kind == 8 {
            return sample;
        }
        let x = sample as f32 * (self.state[7] * PCM_SCALE);
        if kind == 7 {
            let c = self.state[0] * 2.21;
            let q = (1. - self.state[2]) as f64;
            let feedback = q.mul_add(1.4, q / (1. - c as f64)) as f32;
            let first = self.state[5];
            let second = self.state[6];
            let delta = (first - second).mul_add(feedback, x.mul_add(nonlinear(x), -first));
            let first = delta.mul_add(c, first).clamp(-1., 1.);
            let second = (first - second).mul_add(c, second).clamp(-1., 1.);
            self.state[5] = first;
            self.state[6] = second;
            return sat16((second * 32767.) as i32);
        }
        let g = self.state[1];
        let q = if self.state[0] >= 0.4499 {
            1.
        } else {
            self.state[2]
        };
        let two_g = g + g;
        let inv = 1. / q.mul_add(two_g, 1.);
        let gg = (g * g) * inv;
        let ginv = g * inv;
        let invgg = 1. / (gg + 1.);
        let s1 = self.state[3];
        let t = nonlinear(s1);
        let input = nonlinear(x) * x;
        let low = ginv.mul_add(s1, gg.mul_add(input, self.state[4])) * invgg;
        let difference = input - low;
        let band = t.mul_add(s1, g * difference) * inv;
        let unit = band * (q + q);
        self.state[4] = band.mul_add(two_g, self.state[4]);
        let high = difference - unit;
        self.state[3] = t.mul_add(s1, high * two_g);
        let out = match kind {
            1 => {
                if low < -1.95 {
                    -32767.
                } else if low > 1.95 {
                    32767.
                } else {
                    ((low * 4.15) / low.mul_add(low, 4.29)) * 32767.
                }
            }
            2 => high * 32767.,
            3 => band * 32767.,
            4 => unit * 32767.,
            5 => (input - unit) * 32767.,
            6 => (unit + low.mul_add(2., -input)) * 32767.,
            _ => sample as f32,
        };
        sat16(out as i32)
    }
}
