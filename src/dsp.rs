use crate::{
    firmware::{
        self as fw,
        delay::Delay,
        effects::{Compressor, Drive, Ring, Shaper},
        envelope::{Decay, Envelope},
        filter::Filter,
        oscillator::Oscillator,
        transient::Transient,
        OnePole,
    },
    params::*,
};

pub struct Sine(pub [f32; 4097]);
impl Default for Sine {
    fn default() -> Self {
        Self(std::array::from_fn(|i| {
            fw::oscillator::lookup(0, 0, (i as u32).wrapping_shl(20)) as f32 / 32768.
        }))
    }
}
impl Sine {
    #[inline]
    pub fn at(&self, phase: f32) -> f32 {
        let x = (phase - phase.floor()) * 4096.;
        let i = x as usize;
        self.0[i] + (self.0[i + 1] - self.0[i]) * (x - i as f32)
    }
}
#[derive(Clone, Copy)]
pub struct Rng(pub u32);
impl Rng {
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    #[inline]
    pub fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / 16777216.
    }
    #[inline]
    pub fn bipolar(&mut self) -> f32 {
        self.unit() * 2. - 1.
    }
}
#[inline]
pub fn envelope_curve(x: f32, slope: f32) -> f32 {
    let s = slope.clamp(0.005, 0.995);
    let k = (2. * s - 1.) / (1. - s);
    ((k + 1.) * x) / k.mul_add(x.abs(), 1.)
}

#[derive(Clone, Copy)]
pub struct Voice {
    pub p: [f32; PARAMS],
    pub active_track: usize,
    pub meter: f32,
    index: usize,
    oscillators: [Oscillator; 3],
    noise_osc: Oscillator,
    amplitude: Envelope,
    filter_envelope: Envelope,
    assign_envelope: Envelope,
    filter_level: f32,
    assign_level: f32,
    pitch: Decay,
    transient: Transient,
    filter: Filter,
    filter2: Filter,
    modeled_filters: [crate::rustias_filter::Model; 2],
    filter_settings: [[f32; 4]; 2],
    shaper: Shaper,
    extra_drive: Drive,
    velocity: f32,
    note_ratio: f32,
    control: usize,
    amp_previous: f32,
    amp_current: f32,
    lfo_phase: f32,
    lfo_value: f32,
    lfo_random: f32,
    reduction_phase: f32,
    held: i16,
    noise: Rng,
    triggered: bool,
    modulation_amount: f32,
}
impl Voice {
    pub fn new(index: usize) -> Self {
        Self {
            p: kit(0)[index],
            active_track: index,
            meter: 0.,
            index,
            oscillators: [Oscillator::default(); 3],
            noise_osc: Oscillator {
                wave: 4,
                ..Default::default()
            },
            amplitude: Envelope::default(),
            filter_envelope: Envelope::default(),
            assign_envelope: Envelope::default(),
            filter_level: 0.,
            assign_level: 0.,
            pitch: Decay::default(),
            transient: Transient::default(),
            filter: Filter::default(),
            filter2: Filter::default(),
            modeled_filters: [crate::rustias_filter::Model::default(); 2],
            filter_settings: [[0.; 4]; 2],
            shaper: Shaper {
                smooth: OnePole {
                    coefficient: 0.01,
                    state: 0.,
                },
                ..Default::default()
            },
            extra_drive: Drive::default(),
            velocity: 0.,
            note_ratio: 1.,
            control: 0,
            amp_previous: 0.,
            amp_current: 0.,
            lfo_phase: 0.,
            lfo_value: 0.,
            lfo_random: 0.,
            reduction_phase: 1.,
            held: 0,
            noise: Rng(0x9182ace1u32.wrapping_add(index as u32 * 193)),
            triggered: false,
            modulation_amount: 0.,
        }
    }
    pub fn prepare(&mut self, sr: f32) {
        let duration = if self.active_track == 6 {
            self.p[OPEN_DECAY]
        } else {
            self.p[DECAY]
        };
        self.amplitude.attack = if self.p[ATTACK] <= 0. {
            1.
        } else {
            (64. / (self.p[ATTACK] * sr)).min(1.) as f64
        };
        self.amplitude.decay = (64. / (duration * sr)) as f64;
        self.amplitude.repeats = self.p[REPEAT] as u8;
        // Original repeat mode uses attack as the fall time of the repeated pulses.
        if self.amplitude.repeats > 0 {
            self.amplitude.attack = (64. / (self.p[ATTACK].max(0.012) * sr)) as f64;
        }
        self.amplitude.slope(self.p[SLOPE]);
        for (env, attack, decay, slope) in [
            (
                &mut self.filter_envelope,
                FILTER_ENV_ATTACK,
                FILTER_ENV_DECAY,
                FILTER_ENV_SLOPE,
            ),
            (
                &mut self.assign_envelope,
                ASSIGN_ENV_ATTACK,
                ASSIGN_ENV_DECAY,
                ASSIGN_ENV_SLOPE,
            ),
        ] {
            env.attack = if self.p[attack] <= 0. {
                1.
            } else {
                (64. / (self.p[attack] * sr)).min(1.) as f64
            };
            env.decay = (64. / (self.p[decay] * sr)) as f64;
            env.slope(self.p[slope]);
        }
        self.pitch.delta = 64. / (self.p[PITCH_DECAY] * sr);
        let s = self.p[PITCH_SLOPE];
        self.pitch.slope = (2. * s - 1.) / (1. - s);
        self.transient.kind = self.p[TRANSIENT] as u8;
        self.transient.rate = self.p[CLICK_FREQ];
        self.transient.volume = self.p[CLICK];
        let amount = self.p[DRIVE];
        self.shaper.amount = ((amount - 0.0078125) + (amount - 0.0078125)) / (1.0078125 - amount);
        self.extra_drive.kind = (self.p[DISTORTION] as u8).saturating_sub(1);
        self.extra_drive.amount = amount;
        self.extra_drive.tone = 1.;
        self.extra_drive.shape = 0.;
        self.noise_osc.tune(self.p[NOISE_FREQ], sr);
    }
    pub fn trigger(&mut self, track: usize, velocity: f32, note: i32, sr: f32) {
        self.active_track = track;
        self.velocity = velocity.clamp(0., 1.);
        self.note_ratio = 2f32.powf(note as f32 / 12.);
        if self.p[VELOCITY_TARGET] as u8 == 1 {
            self.note_ratio *= 2f32.powf((self.velocity - 1.) * self.p[VELOCITY_AMOUNT] * 2.);
        }
        self.prepare(sr);
        if self.p[VELOCITY_TARGET] as u8 == 4 {
            self.amplitude.decay *=
                2f64.powf(((1. - self.velocity) * self.p[VELOCITY_AMOUNT] * 2.) as f64);
        }
        self.pitch.level = 1.;
        self.amplitude.trigger();
        // Filter and assignable envelopes retrigger independently on every hit.
        self.filter_envelope.level = 0.;
        self.assign_envelope.level = 0.;
        self.filter_envelope.trigger();
        self.assign_envelope.trigger();
        self.transient.trigger();
        let offset = if self.transient.kind == 1 {
            1. - self.p[CLICK]
        } else {
            1.
        };
        for (i, osc) in self.oscillators.iter_mut().enumerate() {
            let offset = if self.index < 3 && i > 0 { 1. } else { offset };
            osc.wave = match i {
                0 => self.p[WAVE] as u8,
                1 => self.p[FM_WAVE] as u8,
                _ => self.p[FM_WAVE2] as u8,
            };
            osc.phase = if self.index >= 4 && i > 0 {
                0
            } else {
                match osc.wave {
                    0 => {
                        if i == 0 && self.index < 3 {
                            offset.mul_add(1072692224., 1024.) as u32
                        } else {
                            (offset * 1072693248.) as u32
                        }
                    }
                    1..=3 => (offset * 1069547520.) as u32,
                    _ => 0,
                }
            };
        }
        if self.index < 3 {
            self.filter.state[3..7].fill(0.);
        }
        if self.p[LFO_RETRIGGER] > 0.5 {
            self.lfo_phase = 0.;
        }
        self.control = 0;
        self.triggered = true;
    }
    fn update_control(&mut self, sr: f32, sine: &Sine) {
        self.lfo_phase += self.p[LFO_RATE] * 64. / sr;
        if self.lfo_phase >= 1. {
            self.lfo_phase -= self.lfo_phase.floor();
            self.lfo_random = self.noise.bipolar();
        }
        self.lfo_value = match self.p[LFO_WAVE] as u8 {
            1 => 1. - 4. * (self.lfo_phase - 0.5).abs(),
            2 => 2. * self.lfo_phase - 1.,
            3 => {
                if self.lfo_phase < 0.5 {
                    1.
                } else {
                    -1.
                }
            }
            4 => self.lfo_random,
            _ => sine.at(self.lfo_phase),
        } * self.p[LFO_DEPTH];
        let eg = self.pitch.tick();
        let pitch_amount = 2f32.powf(self.p[PITCH_AMOUNT] / 12.) - 1.;
        let mut pitch_multiplier = if self.index < 4 {
            pitch_amount.mul_add(eg, 1.)
        } else {
            1.
        };
        if self.transient.kind == 0 {
            pitch_multiplier =
                self.p[CLICK].mul_add(self.transient.pitch(self.p[CLICK_FREQ]), pitch_multiplier);
        }
        let mut base = self.p[PITCH] * self.note_ratio;
        if self.p[LFO_TARGET] as u8 == 0 {
            base *= 1. + self.lfo_value * 0.6;
        }
        let mut fm = self.p[FM_AMOUNT];
        if self.p[LFO_TARGET] as u8 == 2 {
            fm = (fm + self.lfo_value).clamp(0., 1.);
        }
        if self.p[VELOCITY_TARGET] as u8 == 3 {
            fm *= 1. + (self.velocity - 1.) * self.p[VELOCITY_AMOUNT];
        }
        self.modulation_amount = fm;
        self.oscillators[0].wave = self.p[WAVE] as u8;
        self.oscillators[1].wave = self.p[FM_WAVE] as u8;
        self.oscillators[2].wave = self.p[FM_WAVE2] as u8;
        self.oscillators[0].fm = if self.index < 3 { fm * eg } else { 1. };
        self.oscillators[0].tune(base * pitch_multiplier, sr);
        // The two modulators do not follow the carrier's pitch EG.
        self.oscillators[1].tune(base * self.p[FM_RATIO], sr);
        self.oscillators[2].tune(base * self.p[FM_RATIO2], sr);
        let direct_attack = self.amplitude.attack == 1. && matches!(self.amplitude.stage, 1 | 3);
        self.amp_previous = self.amp_current;
        self.amp_current = self.amplitude.tick();
        self.filter_level = self.filter_envelope.tick().clamp(0., 1.);
        self.assign_level = self.assign_envelope.tick().clamp(0., 1.);
        if self.index < 3 && direct_attack {
            self.amp_previous = self.amp_current;
        }
        let mut cutoff = self.p[CUTOFF];
        if self.p[LFO_TARGET] as u8 == 1 {
            cutoff *= 2f32.powf(self.lfo_value * 4.);
        }
        if self.p[VELOCITY_TARGET] as u8 == 2 {
            cutoff *= 2f32.powf((self.velocity - 1.) * self.p[VELOCITY_AMOUNT] * 4.);
        }
        cutoff *= 2f32.powf(self.filter_level * self.p[FILTER_ENV_AMOUNT] / 12.);
        let cutoff2 =
            self.p[CUTOFF2] * 2f32.powf(self.filter_level * self.p[FILTER_ENV_AMOUNT2] / 12.);
        self.filter_settings = [
            [
                cutoff,
                self.p[RESONANCE],
                self.p[FILTER_DRIVE],
                self.p[FILTER_MORPH],
            ],
            [
                cutoff2,
                self.p[RESONANCE2],
                self.p[FILTER_DRIVE2],
                self.p[FILTER_MORPH2],
            ],
        ];
        self.filter
            .configure(cutoff, self.p[RESONANCE], self.p[FILTER_DRIVE], sr);
        self.filter2
            .configure(cutoff2, self.p[RESONANCE2], self.p[FILTER_DRIVE2], sr);
    }
    pub fn envelope_levels(&self) -> [f32; 3] {
        [
            self.amp_current.clamp(0., 1.),
            self.filter_level,
            self.assign_level,
        ]
    }
    fn filter_tick(&mut self, input: i16, slot: usize, sr: f32) -> i16 {
        let kind = if slot == 0 && self.p[FILTER_MODEL] > 0. {
            7 + self.p[FILTER_MODEL] as u8
        } else {
            self.p[if slot == 0 { FILTER } else { FILTER2 }] as u8
        };
        if kind < 8 {
            return if slot == 0 {
                self.filter.tick(input, kind)
            } else {
                self.filter2.tick(input, kind)
            };
        }
        let [cutoff, resonance, drive, morph] = self.filter_settings[slot];
        let sample = self.modeled_filters[slot].sample(
            input as f64 / 32768.,
            kind - 7,
            cutoff as f64,
            resonance as f64,
            drive as f64,
            morph as f64,
            sr as f64,
        );
        fw::sat16((sample * 32768.) as i32)
    }
    #[inline]
    pub fn tick(&mut self, _index: usize, sr: f32, sine: &Sine) -> f32 {
        if self.control == 0 {
            self.update_control(sr, sine);
        }
        let i = self.control;
        self.control = (self.control + 1) % 64;
        if !self.triggered {
            return 0.;
        }
        let width = self.p[PWM];
        let fm = self.modulation_amount;
        let rng = &mut self.noise;
        let parallel_input;
        let mut sample = if self.index == 3 {
            // 0x0801119c: filter the clocked noise alone; mix pitched tone after it.
            let noise = self
                .noise_osc
                .tick(0.9, width, None, || rng.next_u32() as i16);
            let tone =
                self.oscillators[0].tick(1. - self.p[NOISE], width, None, || rng.next_u32() as i16);
            let transient = self.transient.tick() as f32;
            parallel_input =
                fw::sat16((transient + (noise as f32).mul_add(self.p[NOISE], tone as f32)) as i32);
            let noise = self.filter_tick(noise, 0, sr) as f32;
            transient + noise.mul_add(self.p[NOISE], tone as f32)
        } else {
            let m1 = self.oscillators[1].tick(fm, width, None, || rng.next_u32() as i16);
            let signal = if self.index >= 4 {
                let m2 = self.oscillators[2]
                    .tick(self.p[FM_AMOUNT2], width, None, || rng.next_u32() as i16);
                let modulation = fw::sat16(m1 as i32 + m2 as i32);
                let amplitude = if self.index == 5 { 0.5 } else { 1. };
                self.oscillators[0]
                    .tick(amplitude, width, Some(modulation), || rng.next_u32() as i16)
            } else if self.p[FM_MODE] > 0.5 {
                let carrier =
                    self.oscillators[0].tick(1. - fm, width, None, || rng.next_u32() as i16);
                fw::sat16(carrier as i32 + m1 as i32)
            } else {
                self.oscillators[0].tick(1., width, Some(m1), || rng.next_u32() as i16)
            };
            if self.index < 3 {
                let combined = fw::sat16(signal as i32 + self.transient.tick() as i32);
                parallel_input = combined;
                self.filter_tick(combined, 0, sr) as f32
            } else {
                let filtered = self.filter_tick(signal, 0, sr);
                let transient = self.transient.tick();
                parallel_input = fw::sat16(signal as i32 + transient as i32);
                fw::sat16(filtered as i32 + transient as i32) as f32
            }
        };
        if self.p[FILTER2] > 0. {
            sample = if self.p[FILTER_ROUTING] < 0.5 {
                self.filter_tick(fw::sat16(sample as i32), 1, sr) as f32
            } else {
                let second = self.filter_tick(parallel_input, 1, sr) as f32;
                sample + (second - sample) * self.p[FILTER_BALANCE]
            };
        }
        // Drums interpolate EG endpoints over 64 samples; snare/cymbals hold it.
        let amp = if self.index < 3 {
            ((i as f32 * (1. / 63.)) as f64).mul_add(
                (self.amp_current - self.amp_previous) as f64,
                self.amp_previous as f64,
            )
        } else {
            self.amp_current as f64
        };
        let mut output = if self.index < 3 {
            let x = (sample as f64 * amp) as i32 as i16;
            fw::wrap16(x as f32 * self.velocity)
        } else {
            fw::wrap16(sample * (self.velocity * self.amp_current))
        };
        output = match self.p[DISTORTION] as u8 {
            0 => output,
            1 => {
                if self.index < 3 {
                    self.shaper.tick(output)
                } else {
                    self.shaper.tick_static(output)
                }
            }
            _ => self.extra_drive.tick([output, output], 1.)[0],
        };
        self.reduction_phase += 1. / (1. + self.p[REDUCTION] * 63.);
        if self.reduction_phase >= 1. {
            self.reduction_phase -= 1.;
            self.held = output;
        }
        output = self.held;
        let modulation = if self.p[LFO_TARGET] as u8 == 3 {
            1. - self.p[LFO_DEPTH] * 0.5 + self.lfo_value * 0.5
        } else {
            1.
        };
        let out = output as f32 / 32768. * modulation;
        self.meter = self.meter.max(out.abs()) * 0.999;
        out
    }
}

pub struct Effects {
    pub kind: u8,
    pub mix: f32,
    pub time: f32,
    pub feedback: f32,
    pub tone: f32,
    pub drive: f32,
    pub color: f32,
    pub level: f32,
    pub ring_wave: u8,
    pub attack: f32,
    pub release: f32,
    pub threshold: f32,
    pub ratio: f32,
    pub delay_range: f32,
    distortion: Drive,
    ring: Ring,
    compressor: Compressor,
    delay: Delay,
    previous: [u32; 14],
}
impl Effects {
    pub fn new(_sr: f32) -> Self {
        Self {
            kind: 0,
            mix: 0.22,
            time: 0.25,
            feedback: 0.35,
            tone: 0.55,
            drive: 0.25,
            color: 0.,
            level: 1.,
            ring_wave: 0,
            attack: 0.25,
            release: 0.5,
            threshold: 0.75,
            ratio: 4.,
            delay_range: 1.,
            distortion: Drive::default(),
            ring: Ring::default(),
            compressor: Compressor::default(),
            delay: Delay::default(),
            previous: [u32::MAX; 14],
        }
    }
    pub fn clear(&mut self) {
        self.distortion = Drive::default();
        self.ring = Ring::default();
        self.compressor = Compressor::default();
        self.delay.reset();
        self.previous = [u32::MAX; 14];
    }
    fn prepare(&mut self, sr: f32) {
        let current = [
            self.kind as u32,
            self.time.to_bits(),
            self.feedback.to_bits(),
            self.tone.to_bits(),
            self.drive.to_bits(),
            self.color.to_bits(),
            self.level.to_bits(),
            self.ring_wave as u32,
            self.attack.to_bits(),
            self.release.to_bits(),
            self.threshold.to_bits(),
            self.ratio.to_bits(),
            self.delay_range.to_bits(),
            sr.to_bits(),
        ];
        if current == self.previous {
            return;
        }
        self.distortion.kind = match self.kind {
            6 => 1,
            7 => 2,
            _ => 0,
        };
        self.distortion.amount = self.drive;
        self.distortion.shape = self.color;
        self.distortion.tone = self.tone;
        self.distortion.level = self.level;
        self.ring.oscillator.wave = self.ring_wave;
        self.ring.configure(self.time / 0.7 * 880., sr);
        self.compressor.configure(
            self.attack,
            self.release,
            self.threshold,
            self.drive,
            1. / self.ratio,
            sr,
        );
        let ping = self.kind == 5;
        if self.delay.ping_pong != ping {
            self.delay.change_mode(ping);
        }
        self.delay.feedback = self.feedback;
        self.delay.set_tone(self.tone);
        // Preserve time in seconds in the web session; invert the firmware map.
        let r = self.delay_range;
        let hi = (1. - r).mul_add(882.8, r * if ping { 15728.159 } else { 32767. });
        let lo = (1. - r).mul_add(26.484, r * 882.8);
        let normalized = if r > 0. {
            ((self.time * fw::RATE - lo) / (hi - lo) / r).clamp(0., 1.)
        } else {
            0.
        };
        if self.delay.range != r || self.previous[12] != current[12] {
            self.delay.set_range(r);
        }
        if self.previous[1] != current[1]
            || self.previous[0] != current[0]
            || self.previous[12] != current[12]
        {
            self.delay.set_time(normalized);
        }
        self.previous = current;
    }
    #[inline]
    pub fn tick(&mut self, input: [f32; 2], sr: f32, _sine: &Sine) -> [f32; 2] {
        self.tick_pcm(input.map(fw::pcm), sr)
            .map(|x| x as f32 / 32768.)
    }
    #[inline]
    pub fn tick_pcm(&mut self, dry: [i16; 2], sr: f32) -> [i16; 2] {
        self.prepare(sr);
        match self.kind {
            1 | 6 | 7 => self.distortion.tick(dry, self.mix),
            2 => self.ring.tick(dry, self.mix),
            3 => self.compressor.tick(dry, self.mix),
            4 | 5 => self.delay.tick(dry, self.mix),
            _ => dry,
        }
    }
}
