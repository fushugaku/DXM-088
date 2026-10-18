//! DSP translated from LXR-02 1.71. Addresses refer to the mapped firmware image.
//! Kernels retain PCM16 boundaries, ARM conversion semantics and fused operations.
pub mod delay;
pub mod effects;
pub mod envelope;
pub mod filter;
pub mod math;
pub mod oscillator;
pub mod transient;

pub const RATE: f32 = 44140.;
/// Derived from PLLI2S=113 MHz, MCLK=256fs, I2SDIV=5, ODD=0.
/// See reverse/tools/verify_audio_clock.py; RATE above is the firmware DSP macro.
pub const CLOCK: f64 = 44140.625;
pub const BLOCK: usize = 64;
pub const PCM_SCALE: f32 = 1. / 32767.;

#[inline]
pub fn sat16(x: i32) -> i16 {
    x.clamp(-32768, 32767) as i16
}
#[inline]
pub fn pcm(x: f32) -> i16 {
    sat16((x * 32767.) as i32)
}
#[inline]
pub fn wrap16(x: f32) -> i16 {
    x as i32 as i16
}

/// 0x438: the stored double contains an exactly representable float32 result.
#[derive(Clone, Copy, Default, Debug)]
pub struct OnePole {
    pub coefficient: f64,
    pub state: f64,
}
impl OnePole {
    #[inline]
    pub fn tick(&mut self, x: f64) -> f32 {
        let y = (self.coefficient as f32).mul_add((x - self.state) as f32, self.state as f32);
        self.state = y as f64;
        y
    }
}
