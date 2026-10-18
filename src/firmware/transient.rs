//! 0x08012124 (snappy) and 0x08012178 (12 PCM ROM transients).
use super::wrap16;
const SAMPLES: &[u8; 26460] = include_bytes!("../data/transients.i8");
#[derive(Clone, Copy, Debug)]
pub struct Transient {
    pub kind: u8,
    pub position: u32,
    pub rate: f32,
    pub volume: f32,
    pub snappy: f32,
}
impl Default for Transient {
    fn default() -> Self {
        Self {
            kind: 0,
            position: 0x89c00000,
            rate: 1.,
            volume: 0.,
            snappy: 0.,
        }
    }
}
impl Transient {
    pub fn trigger(&mut self) {
        self.position = 0;
        self.snappy = 1.;
    }
    pub fn pitch(&mut self, rate: f32) -> f32 {
        if self.snappy <= 0. {
            return 0.;
        }
        let y = (self.snappy * self.snappy) * 24.;
        self.snappy = (-rate).mul_add(0.2, self.snappy);
        y
    }
    #[inline]
    pub fn tick(&mut self) -> i16 {
        if self.kind < 2 || self.kind > 13 || self.position >= 0x89c00000 {
            return 0;
        }
        let offset = (self.kind as usize - 2) * 2205 + (self.position >> 20) as usize;
        let y = wrap16(((SAMPLES[offset] as i8 as i32) << 8) as f32 * self.volume);
        self.position = self.rate.mul_add(1048576., self.position as f32) as u32;
        y
    }
}
