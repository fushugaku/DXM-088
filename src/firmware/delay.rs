//! Original half-precision delay line, 0x08012bcc / 0x080127d4.
use super::{effects::blend, sat16, wrap16, OnePole, PCM_SCALE};

/// IEEE binary16, round to nearest with ties to even (ARM VCVTB default).
pub fn to_half(x: f32) -> u16 {
    let b = x.to_bits();
    let sign = ((b >> 16) & 0x8000) as u16;
    let exponent = ((b >> 23) & 255) as i32 - 127 + 15;
    let mantissa = b & 0x7fffff;
    if exponent >= 31 {
        return sign
            | 0x7c00
            | if mantissa != 0 && b & 0x7f800000 == 0x7f800000 {
                0x200
            } else {
                0
            };
    }
    if exponent <= 0 {
        if exponent < -10 {
            return sign;
        }
        let m = mantissa | 0x800000;
        let shift = (14 - exponent) as u32;
        let q = m >> shift;
        let remainder = m & ((1 << shift) - 1);
        let halfway = 1 << (shift - 1);
        return sign
            | (q + u32::from(remainder > halfway || (remainder == halfway && q & 1 != 0))) as u16;
    }
    let q = ((exponent as u32) << 10) | (mantissa >> 13);
    let remainder = mantissa & 0x1fff;
    sign | (q + u32::from(remainder > 0x1000 || (remainder == 0x1000 && q & 1 != 0))) as u16
}
pub fn from_half(h: u16) -> f32 {
    let sign = ((h & 0x8000) as u32) << 16;
    let exponent = (h >> 10) & 31;
    let mantissa = (h & 1023) as u32;
    if exponent == 0 {
        return if mantissa == 0 {
            f32::from_bits(sign)
        } else {
            (mantissa as f32 * (1. / 16777216.)).copysign(f32::from_bits(sign | 0x3f800000))
        };
    }
    f32::from_bits(
        sign | if exponent == 31 {
            0x7f800000 | (mantissa << 13)
        } else {
            ((exponent as u32 + 112) << 23) | (mantissa << 13)
        },
    )
}
#[derive(Clone, Debug)]
pub struct Delay {
    pub line: Vec<u16>,
    pub read: u32,
    pub next: u32,
    pub write: u32,
    pub fraction: f32,
    pub old_fraction: f32,
    pub fade: f32,
    pub fade_step: f32,
    pub last: [i16; 4],
    pub feedback: f32,
    pub range: f32,
    pub time: f32,
    pub pending_time: f32,
    pub pending: bool,
    pub tone: [OnePole; 2],
    pub ping_pong: bool,
}
impl Default for Delay {
    fn default() -> Self {
        Self::with_line(vec![0; 32768])
    }
}
impl Delay {
    fn with_line(line: Vec<u16>) -> Self {
        Self {
            line,
            read: 0,
            next: 0,
            write: 0,
            fraction: 0.,
            old_fraction: 0.,
            fade: 0.,
            fade_step: f32::from_bits(0x3b147919),
            last: [0; 4],
            feedback: 0.35,
            range: 1.,
            time: 0.,
            pending_time: 0.,
            pending: false,
            tone: [OnePole {
                coefficient: 0.5,
                state: 0.,
            }; 2],
            ping_pong: false,
        }
    }
}
impl Delay {
    /// Reset control and audio state while retaining the allocated delay memory.
    pub fn reset(&mut self) {
        self.line.fill(0);
        let line = std::mem::take(&mut self.line);
        *self = Self::with_line(line);
    }
    pub fn clear(&mut self) {
        self.line.fill(0);
        self.tone[0].state = 0.;
        self.tone[1].state = 0.;
    }
    pub fn change_mode(&mut self, ping_pong: bool) {
        self.clear();
        self.ping_pong = ping_pong;
        self.read = 0;
        self.next = 0;
        self.write = 0;
        self.fraction = 0.;
        self.old_fraction = 0.;
        self.fade = 0.;
        self.pending = false;
        self.last = [0; 4];
    }
    pub fn set_tone(&mut self, x: f32) {
        self.tone[0].coefficient = (x * x) as f64;
        self.tone[1].coefficient = (x * x) as f64;
    }
    fn mask(&self) -> u32 {
        if self.ping_pong {
            0x3fff
        } else {
            0x7fff
        }
    }
    fn begin_time(&mut self, time: f32) {
        self.time = time;
        let r = self.range;
        let p = time * r;
        let max = if self.ping_pong {
            15728.159_f32
        } else {
            32767.
        };
        let hi = (1. - r).mul_add(882.8, r * max);
        let lo = (1. - r).mul_add(26.484, r * 882.8);
        let samples = lo.mul_add(1. - p, p * hi);
        let whole = samples as u32;
        self.next = self.write.wrapping_sub(1).wrapping_sub(whole) & self.mask();
        self.fraction = samples - whole as f32;
        self.fade_step = f32::from_bits(0x3b147919);
        self.fade = 1.;
    }
    pub fn set_time(&mut self, time: f32) {
        if self.fade > 0. {
            self.pending_time = time;
            self.pending = true;
        } else {
            self.begin_time(time);
        }
    }
    pub fn set_range(&mut self, range: f32) {
        self.range = range;
        self.set_time(self.time);
    }
    #[inline]
    fn interpolate(&self, index: u32, fraction: f32, channel: usize) -> i16 {
        let offset = if channel == 1 { 0x3fff } else { 0 };
        let a = from_half(self.line[index as usize + offset]);
        let b = from_half(self.line[((index + 1) & self.mask()) as usize + offset]);
        wrap16(a.mul_add(1. - fraction, b * fraction) * 32767.)
    }
    #[inline]
    fn store(&mut self, index: usize, value: i16) {
        self.line[index] = if value == i16::MIN {
            0xbc00
        } else {
            to_half(value as f32 * PCM_SCALE)
        };
    }
    #[inline]
    pub fn tick(&mut self, input: [i16; 2], mix: f32) -> [i16; 2] {
        let channels = if self.ping_pong { 2 } else { 1 };
        let mut samples = [0i16; 2];
        if self.fade > 0. {
            let fade = self.fade;
            for (c, sample) in samples.iter_mut().enumerate().take(channels) {
                let old = self.interpolate(self.read, self.old_fraction, c);
                let new = self.interpolate(self.next, self.fraction, c);
                self.last[c * 2] = old;
                self.last[c * 2 + 1] = new;
                *sample = ((new as f64).mul_add(1. - fade as f64, (old as f32 * fade) as f64))
                    as i32 as i16;
            }
            self.read = (self.read + 1) & self.mask();
            self.next = (self.next + 1) & self.mask();
            self.fade -= self.fade_step;
            if self.fade <= 0. {
                self.read = self.next;
                self.old_fraction = self.fraction;
            }
        } else {
            for (c, sample) in samples.iter_mut().enumerate().take(channels) {
                *sample = self.interpolate(self.read, self.old_fraction, c);
                self.last[c * 2] = *sample;
            }
            self.read = (self.read + 1) & self.mask();
            if self.pending {
                self.pending = false;
                self.begin_time(self.pending_time);
            }
        }
        let left = wrap16(self.tone[0].tick(samples[0] as f64));
        let right = if self.ping_pong {
            wrap16(self.tone[1].tick(samples[1] as f64))
        } else {
            left
        };
        if self.ping_pong {
            let l = sat16(input[0] as i32 + wrap16(left as f32 * self.feedback) as i32);
            let r = sat16(input[1] as i32 + wrap16(right as f32 * self.feedback) as i32);
            self.store(self.write as usize + 0x3fff, l);
            self.store(self.write as usize, r);
        } else {
            let dry = sat16(input[0] as i32 + input[1] as i32);
            let write = sat16(dry as i32 + wrap16(left as f32 * self.feedback) as i32);
            self.store(self.write as usize, write);
        }
        self.write = (self.write + 1) & self.mask();
        [blend(input[0], left, mix), blend(input[1], right, mix)]
    }
}
