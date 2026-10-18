//! 0x1098 (oscillator), 0xb28 / 0x08010c80 (phase modulation), 0x08010da4 (tuning).
use super::{sat16, wrap16, RATE};
const WAVES: &[u8; 75776] = include_bytes!("../data/lxr02-waves.bin");
const GUARDS: &[u8; 8] = include_bytes!("../data/wave-guards.bin");

#[derive(Clone, Copy, Debug)]
pub struct Oscillator {
    pub phase: u32,
    pub increment: u32,
    pub wave: u8,
    pub bank: u8,
    pub held: i16,
    pub fm: f32,
}
impl Default for Oscillator {
    fn default() -> Self {
        Self {
            phase: 0,
            increment: 0,
            wave: 0,
            bank: 4,
            held: 0,
            fm: 0.,
        }
    }
}

#[inline]
fn read(base: usize, bank: usize, index: usize, sine: bool, wave: u8) -> i16 {
    let size = if sine { 4096 } else { 11 * 1024 };
    let i = bank * 1024 + index;
    if i == size {
        let g = wave as usize * 2;
        return i16::from_le_bytes([GUARDS[g], GUARDS[g + 1]]);
    }
    let p = base + i * 2;
    i16::from_le_bytes([WAVES[p], WAVES[p + 1]])
}

#[inline]
pub fn lookup(wave: u8, bank: u8, phase: u32) -> i16 {
    let (base, shift) = match wave {
        0 => (0, 20),
        1 => (8192, 22),
        2 | 5 => (30720, 22),
        _ => (53248, 22),
    };
    let sine = wave == 0;
    let bank = if sine { 0 } else { bank.min(10) as usize };
    let index = (phase >> shift) as usize;
    let frac = phase & ((1 << shift) - 1);
    let a = read(base, bank, index, sine, if wave == 5 { 2 } else { wave });
    let b = read(
        base,
        bank,
        index + 1,
        sine,
        if wave == 5 { 2 } else { wave },
    );
    // The compiler uses SMULL followed by an arithmetic shift, then SXTH.
    (a as i64 + (((b as i64 - a as i64) * frac as i64) >> shift)) as i16
}

impl Oscillator {
    pub fn tune(&mut self, hz: f32, sample_rate: f32) {
        let hz = hz * (RATE / sample_rate);
        self.increment = (hz * 97303.29_f32) as u32;
        if matches!(self.wave, 1 | 2 | 3 | 5) {
            let x = (hz * 0.0022727272_f32) as u32;
            let exponent = (31i32 - x.leading_zeros() as i32) as u8 as u32;
            // UMULL + UBFX #3,#8, including the byte wrap when x == 0.
            self.bank = ((((exponent * 12 + 70) as u64 * 0xaaaaaaab) >> 35) as u8).min(10);
        }
    }
    #[inline]
    pub fn tick(
        &mut self,
        amplitude: f32,
        width: f32,
        modulation: Option<i16>,
        random: impl FnOnce() -> i16,
    ) -> i16 {
        if self.wave == 4 {
            let (phase, carry) = self.phase.overflowing_add(self.increment);
            self.phase = phase;
            if carry {
                self.held = random();
            }
            return wrap16(self.held as f32 * amplitude);
        }
        let mut p = self.phase;
        if let Some(m) = modulation {
            let shift = if self.wave == 0 { 17 } else { 19 };
            // VCVT.U32.F32 deliberately clips negative modulation to zero.
            p = p.wrapping_add(((m as f32 * self.fm) as u32).wrapping_shl(shift));
        }
        let sample = if self.wave == 5 {
            let offset = width * 2147483648.0_f32;
            let second = (p as f32 + offset) as u32;
            sat16(lookup(2, self.bank, p) as i32 - lookup(2, self.bank, second) as i32)
        } else {
            lookup(self.wave, self.bank, p)
        };
        self.phase = self.phase.wrapping_add(self.increment);
        if self.wave == 5 || (self.wave != 0 && modulation.is_some()) {
            self.held = sample;
        }
        wrap16(sample as f32 * amplitude)
    }
}
