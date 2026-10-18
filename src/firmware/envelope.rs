//! Control-rate envelopes: 0x4bc and 0x1ca8, original block size 64.
#[derive(Clone, Copy, Debug, Default)]
pub struct Decay {
    pub delta: f32,
    pub slope: f32,
    pub level: f32,
}
impl Decay {
    pub fn tick(&mut self) -> f32 {
        if self.level <= 0. {
            self.level = 0.;
            return 0.;
        }
        self.level -= self.delta;
        ((self.slope + 1.) * self.level) / self.slope.mul_add(self.level.abs(), 1.)
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Envelope {
    pub attack: f64,
    pub decay: f64,
    pub decay_slope: f64,
    pub attack_slope: f64,
    pub repeats: u8,
    pub remaining: u8,
    pub level: f64,
    pub stage: u8,
}
impl Envelope {
    pub fn trigger(&mut self) {
        self.remaining = self.repeats;
        if self.repeats > 0 {
            self.stage = 3;
            self.level = 1.;
        } else {
            self.stage = 1;
        }
    }
    pub fn tick(&mut self) -> f32 {
        let k = match self.stage {
            1 => {
                self.level += self.attack;
                if self.level >= 1. {
                    self.level = 1.;
                    self.stage = 2;
                }
                self.attack_slope
            }
            2 => {
                if self.level <= 0. {
                    self.level = 0.;
                    return 0.;
                }
                self.level -= self.decay;
                self.decay_slope
            }
            3 => {
                if self.level <= 0. {
                    self.remaining = self.remaining.wrapping_sub(1);
                    if self.remaining == 0 {
                        self.stage = 2;
                    }
                    self.level = 1.;
                } else {
                    self.level -= self.attack;
                }
                self.decay_slope
            }
            _ => return 0.,
        };
        ((self.level + k * self.level) / k.mul_add(self.level.abs(), 1.)) as f32
    }
    pub fn slope(&mut self, normalized: f32) {
        let x = ((normalized - 0.5) * 2.) as f64;
        self.decay_slope = (x * 2.) / (1. - x);
        self.attack_slope = (-x * 2.) / (1. + x);
    }
}
