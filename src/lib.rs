//! LXR-02 1.71 DSP port with a Rust sequencer and browser interface.
//! Isolated DSP kernels are checked against original ARM execution fixtures.
pub mod dsp;
pub mod firmware;
pub mod params;
pub mod performance;
mod resampler;
pub mod sequencer;

use dsp::{Effects, Rng, Sine, Voice};
use params::*;
use performance::*;
use sequencer::*;
use std::cell::RefCell;

pub const MAX_FRAMES: usize = 4096;
/// Main stereo pair followed by seven stereo drum outputs.
pub const OUTPUT_CHANNELS: usize = 16;
#[no_mangle]
pub extern "C" fn lxr_pattern_count() -> u32 {
    PATTERNS as u32
}

pub struct Engine {
    pub sample_rate: f32,
    pub params: [[f32; PARAMS]; VOICES],
    pub patterns: Box<[Pattern; PATTERNS]>,
    pub pattern: usize,
    pub queued_pattern: Option<usize>,
    pub playing: bool,
    pub bpm: f32,
    pub swing: f32,
    pub tick_count: u64,
    pub positions: [i32; TRACKS],
    pub levels: [f32; TRACKS],
    pub muted: [bool; TRACKS],
    pub solo: [bool; TRACKS],
    pub meters: [f32; TRACKS],
    pub peak: [f32; 2],
    pub master: f32,
    pub morph: f32,
    pub morph_target: usize,
    pub fx: Effects,
    pub global_fx: [f32; 15],
    pub global_morph: [f32; 2],
    pub scenes: Box<[Scene; PATTERNS]>,
    pub samples: [Sample; TRACKS],
    pub macros: [f32; 4],
    pub global_macros: [f32; 4],
    pub routes: [Route; ROUTES],
    pub velocity_depth: f32,
    pub performance_velocity: f32,
    pub pressure: f32,
    pub modwheel: f32,
    pub fill: bool,
    pub lfo_periods: [f32; 2],
    pub switch_quant: u8,
    beat: f64,
    clock_valid: bool,
    clocks: [TrackClock; TRACKS],
    pending_switch: Option<(usize, f64)>,
    mod_counter: u32,
    frame_count: u64,
    hits: [Hit; 512],
    hit_count: usize,
    voices: [Voice; VOICES],
    sine: Sine,
    locks: [[Lock; 2]; VOICES],
    pan: [[f32; 2]; VOICES],
    smooth_levels: [f32; TRACKS],
    smooth_master: f32,
    countdown: f64,
    ratchets: [Ratchet; TRACKS],
    rng: Rng,
    resampler: resampler::Resampler,
    output: Vec<f32>,
    pub capture_stems: bool,
    stem_output: Vec<f32>,
}
impl Engine {
    pub fn new(sample_rate: f32) -> Self {
        let sr = if sample_rate.is_finite() {
            sample_rate.clamp(8000., 192000.)
        } else {
            44100.
        };
        let mut e = Self {
            sample_rate: sr,
            params: kit(0),
            patterns: demo_patterns(),
            pattern: 0,
            queued_pattern: None,
            playing: false,
            bpm: 128.,
            swing: 0.,
            tick_count: 0,
            positions: [-1; TRACKS],
            levels: [0.85; TRACKS],
            muted: [false; TRACKS],
            solo: [false; TRACKS],
            meters: [0.; TRACKS],
            peak: [0.; 2],
            master: 0.8,
            morph: 0.,
            morph_target: 1,
            fx: Effects::new(sr),
            global_fx: [
                0., 0.22, 0.25, 0.35, 0.55, 0.25, 0.8, 0., 1., 0., 0.25, 0.5, 0.75, 4., 1.,
            ],
            global_morph: [0., 1.],
            scenes: vec![Scene::default(); PATTERNS]
                .into_boxed_slice()
                .try_into()
                .unwrap_or_else(|_| unreachable!()),
            samples: std::array::from_fn(|_| Sample::default()),
            macros: [0.; 4],
            global_macros: [0.; 4],
            routes: [Route::default(); ROUTES],
            velocity_depth: 0.,
            performance_velocity: 1.,
            pressure: 0.,
            modwheel: 0.,
            fill: false,
            lfo_periods: [4., 1.],
            switch_quant: 0,
            beat: 0.,
            clock_valid: false,
            clocks: [TrackClock::default(); TRACKS],
            pending_switch: None,
            mod_counter: 0,
            frame_count: 0,
            hits: [Hit::default(); 512],
            hit_count: 0,
            voices: std::array::from_fn(Voice::new),
            sine: Sine::default(),
            locks: [[Lock::default(); 2]; VOICES],
            pan: [[0.; 2]; VOICES],
            smooth_levels: [0.85; TRACKS],
            smooth_master: 0.8,
            countdown: 0.,
            ratchets: [Ratchet::default(); TRACKS],
            rng: Rng(0x72513291),
            resampler: resampler::Resampler::new(sr),
            output: vec![0.; MAX_FRAMES * 2],
            capture_stems: false,
            stem_output: vec![0.; MAX_FRAMES * (OUTPUT_CHANNELS - 2)],
        };
        e.global_fx = e.fx_values();
        e.refresh();
        e
    }
    fn refresh(&mut self) {
        let target = kit(self.morph_target);
        for (v, target_params) in target.iter().enumerate() {
            let scene = &self.scenes[self.pattern];
            let mut p = if scene.enabled {
                scene.params[v]
            } else {
                self.params[v]
            };
            for i in 0..PARAMS {
                p[i] = if DISCRETE.contains(&i) {
                    if self.morph >= 0.5 {
                        target_params[i]
                    } else {
                        p[i]
                    }
                } else {
                    p[i] + (target_params[i] - p[i]) * self.morph
                };
            }
            for route in self.routes {
                if route.voice as usize == v
                    && (route.param as usize) < PARAMS
                    && route.amount != 0.
                {
                    let source = match route.source {
                        0..=3 => self.macros[route.source as usize],
                        4 => self.performance_velocity,
                        5 => self.pressure,
                        6 => self.modwheel,
                        7 | 8 => (self.beat as f32 * std::f32::consts::TAU
                            / self.lfo_periods[(route.source - 7) as usize].max(0.0625))
                        .sin(),
                        _ => 0.,
                    };
                    let id = route.param as usize;
                    p[id] = modulated(id, p[id], source * route.amount);
                }
            }
            for lock in self.locks[v] {
                if (lock.param as usize) < PARAMS {
                    p[lock.param as usize] = lock.value;
                }
            }
            self.voices[v].p = p;
            self.voices[v].prepare(firmware::RATE);
            // Equal-power panning confirmed in the firmware's trigger routines.
            self.pan[v] = [
                ((1. - p[PAN]) * 0.5).sqrt() * p[LEVEL],
                ((1. + p[PAN]) * 0.5).sqrt() * p[LEVEL],
            ];
        }
    }
    pub fn set_param(&mut self, voice: usize, param: usize, value: f32) {
        if voice >= VOICES {
            return;
        }
        if let Some(v) = constrain(param, value) {
            self.params[voice][param] = v;
            if self.scenes[self.pattern].enabled {
                self.scenes[self.pattern].params[voice][param] = v;
            }
            for lock in &mut self.locks[voice] {
                if lock.param as usize == param {
                    *lock = Lock::default();
                }
            }
            self.refresh();
        }
    }
    pub fn load_kit(&mut self, index: usize) {
        self.params = kit(index);
        if self.scenes[self.pattern].enabled {
            self.scenes[self.pattern].params = self.params;
        }
        self.morph = 0.;
        self.global_morph[0] = 0.;
        if self.scenes[self.pattern].enabled {
            self.scenes[self.pattern].morph = 0.;
        }
        self.locks = [[Lock::default(); 2]; VOICES];
        self.refresh();
    }
    pub fn set_morph(&mut self, value: f32, target: usize) {
        if value.is_finite() {
            self.morph = value.clamp(0., 1.);
            self.morph_target = target.min(4);
            self.global_morph = [self.morph, self.morph_target as f32];
            if self.scenes[self.pattern].enabled {
                self.scenes[self.pattern].morph = self.morph;
                self.scenes[self.pattern].morph_target = self.morph_target;
            }
            self.refresh();
        }
    }
    pub fn tempo(&mut self, bpm: f32, swing: f32) {
        if bpm.is_finite() && swing.is_finite() {
            self.bpm = bpm.clamp(30., 300.);
            self.swing = swing.clamp(0., 0.5);
        }
    }
    pub fn play(&mut self) {
        self.playing = true;
    }
    pub fn stop(&mut self) {
        self.playing = false;
        self.beat = 0.;
        self.clock_valid = false;
        self.pending_switch = None;
        self.queued_pattern = None;
        self.tick_count = 0;
        self.countdown = 0.;
        self.positions = [-1; TRACKS];
        self.ratchets = [Ratchet::default(); TRACKS];
        self.locks = [[Lock::default(); 2]; VOICES];
        self.refresh();
        if let Some(p) = self.queued_pattern.take() {
            self.pattern = p;
        }
    }
    pub fn panic(&mut self) {
        self.stop();
        self.voices = std::array::from_fn(Voice::new);
        for sample in &mut self.samples {
            sample.stop();
        }
        self.locks = [[Lock::default(); 2]; VOICES];
        self.fx.clear();
        self.resampler.clear();
        self.refresh();
        self.peak = [0.; 2];
        self.meters = [0.; TRACKS];
    }
    pub fn select_pattern(&mut self, p: usize) {
        if p >= PATTERNS {
            return;
        }
        if p == self.pattern {
            self.pending_switch = None;
            self.queued_pattern = None;
            return;
        }
        if !self.playing || self.switch_quant == 0 {
            self.immediate_pattern(p);
        } else {
            let unit = if self.switch_quant == 2 { 4. } else { 0.25 };
            self.pending_switch = Some((p, ((self.beat / unit + 1e-8).floor() + 1.) * unit));
            self.queued_pattern = Some(p);
        }
    }
    pub fn set_step(
        &mut self,
        p: usize,
        t: usize,
        s: usize,
        velocity: f32,
        probability: f32,
        note: i32,
        ratchet: u32,
    ) {
        if p >= PATTERNS
            || t >= TRACKS
            || s >= STEPS
            || !velocity.is_finite()
            || !probability.is_finite()
        {
            return;
        }
        let step = &mut self.patterns[p].tracks[t].steps[s];
        step.velocity = velocity.clamp(0., 1.);
        step.probability = probability.clamp(0., 1.);
        step.note = note.clamp(-36, 36) as i8;
        step.ratchet = ratchet.clamp(1, 4) as u8;
    }
    pub fn set_lock(
        &mut self,
        p: usize,
        t: usize,
        s: usize,
        slot: usize,
        param: usize,
        value: f32,
    ) {
        if p >= PATTERNS || t >= TRACKS || s >= STEPS || slot > 1 {
            return;
        }
        if param == 255 {
            self.patterns[p].tracks[t].steps[s].locks[slot] = Lock::default();
        } else if let Some(value) = constrain(param, value) {
            self.patterns[p].tracks[t].steps[s].locks[slot] = Lock {
                param: param as u8,
                value,
            };
        }
    }
    pub fn set_length(&mut self, p: usize, t: usize, length: usize) {
        if p < PATTERNS && t < TRACKS {
            self.patterns[p].tracks[t].length = length.clamp(1, STEPS);
        }
    }
    pub fn generate(&mut self, p: usize, t: usize, pulses: usize, length: usize, rotation: usize) {
        if p < PATTERNS && t < TRACKS {
            euclidean(&mut self.patterns[p].tracks[t], pulses, length, rotation);
        }
    }
    pub fn trigger(&mut self, t: usize, velocity: f32, note: i32) {
        if t < TRACKS && velocity.is_finite() {
            let v = t.min(5);
            self.samples[t].trigger(velocity, note);
            if t == 5 {
                self.samples[6].stop();
            } else if t == 6 {
                self.samples[5].stop();
            }
            if self.hit_count < self.hits.len() {
                self.hits[self.hit_count] = Hit {
                    frame: self.frame_count,
                    track: t as u8,
                    note: note.clamp(-36, 36) as i8,
                    velocity,
                };
                self.hit_count += 1;
            }
            self.voices[v].trigger(t, velocity, note.clamp(-36, 36), firmware::RATE);
        }
    }
    fn fire_step(&mut self, tick: i64, duration: f64) {
        for t in 0..TRACKS {
            let position =
                tick.rem_euclid(self.patterns[self.pattern].tracks[t].length as i64) as usize;
            self.positions[t] = position as i32;
            let s = self.patterns[self.pattern].tracks[t].steps[position];
            self.ratchets[t] = Ratchet::default();
            if s.velocity > 0. && self.rng.unit() < s.probability && !self.muted[t] {
                self.locks[t.min(5)] = s.locks;
                self.refresh();
                self.trigger(t, s.velocity, s.note as i32);
                if s.ratchet > 1 {
                    self.ratchets[t] = Ratchet {
                        remaining: s.ratchet - 1,
                        countdown: duration / s.ratchet as f64,
                        interval: duration / s.ratchet as f64,
                        velocity: s.velocity,
                        note: s.note,
                    };
                }
            }
        }
    }
    /// A host clock calls this at each sixteenth, retaining the original ratchets.
    pub fn external_step(&mut self, tick: i64, duration: f64) {
        self.playing = true;
        self.fire_step(tick, duration.max(1.));
        self.tick_count = tick.max(0) as u64 + 1;
    }
    /// MIDI changes the active pattern immediately, without the browser's bar queue.
    pub fn immediate_pattern(&mut self, pattern: usize) {
        if pattern < PATTERNS {
            self.pattern = pattern;
            self.queued_pattern = None;
            self.pending_switch = None;
            self.clock_valid = false;
            if self.scenes[pattern].enabled {
                let scene = self.scenes[pattern];
                for (i, value) in scene.fx.into_iter().enumerate() {
                    self.set_fx_raw(i, value);
                }
                self.macros = scene.macros;
                self.morph = scene.morph;
                self.morph_target = scene.morph_target;
            } else {
                for (i, value) in self.global_fx.into_iter().enumerate() {
                    self.set_fx_raw(i, value);
                }
                self.morph = self.global_morph[0];
                self.morph_target = self.global_morph[1] as usize;
                self.macros = self.global_macros;
            }
            self.ratchets = [Ratchet::default(); TRACKS];
            self.locks = [[Lock::default(); 2]; VOICES];
            self.refresh();
        }
    }
    /// No allocation in the render loop. Stereo interleaved float output.
    pub fn render(&mut self, frames: usize) -> &[f32] {
        self.render_clocked(frames, true)
    }
    /// Render DSP and ratchets while an external host supplies sequencer steps.
    pub fn render_external(&mut self, frames: usize) -> &[f32] {
        self.render_clocked(frames, false)
    }
    fn render_clocked(&mut self, frames: usize, internal_clock: bool) -> &[f32] {
        let n = frames.min(MAX_FRAMES);
        let has_solo = self.solo.iter().any(|s| *s);
        for frame in 0..n {
            if self.playing {
                if internal_clock {
                    self.sequence_frame();
                }
                for t in 0..TRACKS {
                    let r = &mut self.ratchets[t];
                    if r.remaining > 0 {
                        r.countdown -= 1.;
                        if r.countdown <= 0. {
                            r.remaining -= 1;
                            r.countdown += r.interval;
                            let velocity = r.velocity;
                            let note = r.note;
                            self.trigger(t, velocity, note as i32);
                        }
                    }
                }
            }
            let out = if self.resampler.bypass() {
                self.dsp_frame(has_solo)
            } else {
                self.resampler.phase += self.resampler.step;
                while self.resampler.phase >= 1. {
                    self.resampler.phase -= 1.;
                    let sample = self.dsp_frame(has_solo);
                    self.resampler.push(sample);
                }
                self.resampler.output(if self.capture_stems {
                    OUTPUT_CHANNELS
                } else {
                    2
                })
            };
            if self.playing && internal_clock {
                self.beat += self.bpm as f64 / (60. * self.sample_rate as f64);
            }
            self.frame_count = self.frame_count.wrapping_add(1);
            for (c, &x) in out[..2].iter().enumerate() {
                self.output[frame * 2 + c] = x;
                self.peak[c] = self.peak[c].max(x.abs()) * 0.9995;
            }
            if self.capture_stems {
                let at = frame * (OUTPUT_CHANNELS - 2);
                self.stem_output[at..at + OUTPUT_CHANNELS - 2].copy_from_slice(&out[2..]);
            }
        }
        &self.output[..n * 2]
    }
    /// Post voice processing, level, pan and master gain; before shared master FX.
    pub fn stems(&self, frames: usize) -> &[f32] {
        &self.stem_output[..frames.min(MAX_FRAMES) * (OUTPUT_CHANNELS - 2)]
    }
    fn dsp_frame(&mut self, has_solo: bool) -> [f32; OUTPUT_CHANNELS] {
        if self.mod_counter == 0 && self.routes.iter().any(|r| r.amount != 0.) {
            self.refresh();
        }
        self.mod_counter = (self.mod_counter + 1) % 64;
        let scene = &self.scenes[self.pattern];
        let (levels, muted, solo) = if scene.enabled {
            (scene.levels, scene.muted, scene.solo)
        } else {
            (self.levels, self.muted, self.solo)
        };
        let has_solo = has_solo || solo.iter().any(|s| *s);
        for t in 0..TRACKS {
            let target = if muted[t] || (has_solo && !solo[t]) {
                0.
            } else {
                levels[t]
            };
            self.smooth_levels[t] += (target - self.smooth_levels[t]) * 0.01;
            self.meters[t] *= 0.9985;
        }
        self.smooth_master += (self.master - self.smooth_master) * 0.004;
        let mut mixed = [0i16; 2];
        let mut stems = [[0i16; 2]; TRACKS];
        for v in 0..VOICES {
            let x = self.voices[v].tick(v, firmware::RATE, &self.sine);
            let t = self.voices[v].active_track;
            let x = if self.samples[t].replace && !self.samples[t].pcm.is_empty() {
                0.
            } else {
                x
            };
            let a = x * self.smooth_levels[t];
            self.meters[t] = self.meters[t].max(a.abs());
            for (c, m) in mixed.iter_mut().enumerate() {
                let contribution =
                    firmware::sat16(((a * self.pan[v][c] * self.smooth_master) * 32768.) as i32);
                *m = m.saturating_add(contribution);
                if self.capture_stems {
                    stems[t][c] = stems[t][c].saturating_add(contribution);
                }
            }
        }
        for t in 0..TRACKS {
            let a = self.samples[t].tick() * self.smooth_levels[t];
            self.meters[t] = self.meters[t].max(a.abs());
            for (c, m) in mixed.iter_mut().enumerate() {
                let contribution = firmware::sat16(
                    (a * self.pan[t.min(5)][c] * self.smooth_master * 32768.) as i32,
                );
                *m = m.saturating_add(contribution);
                if self.capture_stems {
                    stems[t][c] = stems[t][c].saturating_add(contribution);
                }
            }
        }
        let mut out = [0.; OUTPUT_CHANNELS];
        let main = self.fx.tick_pcm(mixed, firmware::RATE);
        for c in 0..2 {
            out[c] = main[c] as f32 / 32768.;
        }
        if self.capture_stems {
            for t in 0..TRACKS {
                for c in 0..2 {
                    out[2 + t * 2 + c] = stems[t][c] as f32 / 32768.;
                }
            }
        }
        out
    }
    pub fn fx_values(&self) -> [f32; 15] {
        [
            self.fx.kind as f32,
            self.fx.mix,
            self.fx.time,
            self.fx.feedback,
            self.fx.tone,
            self.fx.drive,
            self.master,
            self.fx.color,
            self.fx.level,
            self.fx.ring_wave as f32,
            self.fx.attack,
            self.fx.release,
            self.fx.threshold,
            self.fx.ratio,
            self.fx.delay_range,
        ]
    }
    pub fn capture_scene(&mut self, p: usize, enabled: bool) {
        if p >= PATTERNS {
            return;
        }
        self.scenes[p] = Scene {
            enabled,
            params: if self.scenes[self.pattern].enabled {
                self.scenes[self.pattern].params
            } else {
                self.params
            },
            fx: self.fx_values(),
            levels: self.effective_levels(),
            muted: self.effective_muted(),
            solo: self.effective_solo(),
            macros: self.macros,
            morph: self.morph,
            morph_target: self.morph_target,
        };
    }
    pub fn effective_levels(&self) -> [f32; TRACKS] {
        if self.scenes[self.pattern].enabled {
            self.scenes[self.pattern].levels
        } else {
            self.levels
        }
    }
    pub fn effective_muted(&self) -> [bool; TRACKS] {
        if self.scenes[self.pattern].enabled {
            self.scenes[self.pattern].muted
        } else {
            self.muted
        }
    }
    pub fn effective_solo(&self) -> [bool; TRACKS] {
        if self.scenes[self.pattern].enabled {
            self.scenes[self.pattern].solo
        } else {
            self.solo
        }
    }
    pub fn set_mix(&mut self, t: usize, kind: usize, value: f32) {
        if t >= TRACKS || !value.is_finite() {
            return;
        }
        let scene = &mut self.scenes[self.pattern];
        match kind {
            0 => {
                self.levels[t] = value.clamp(0., 1.25);
                if scene.enabled {
                    scene.levels[t] = self.levels[t];
                }
            }
            1 => {
                self.muted[t] = value >= 0.5;
                if scene.enabled {
                    scene.muted[t] = self.muted[t];
                }
            }
            2 => {
                self.solo[t] = value >= 0.5;
                if scene.enabled {
                    scene.solo[t] = self.solo[t];
                }
            }
            _ => {}
        }
    }
    pub fn track_settings(
        &mut self,
        p: usize,
        t: usize,
        division: u8,
        delay: f32,
        groove: u8,
        amount: f32,
    ) {
        if p < PATTERNS && t < TRACKS && delay.is_finite() && amount.is_finite() {
            let track = &mut self.patterns[p].tracks[t];
            let changed = track.division != division.min(12);
            track.division = division.min(12);
            track.delay = delay.clamp(-0.49, 0.49);
            track.groove = groove.min(4);
            track.groove_amount = amount.clamp(0., 1.);
            if self.clock_valid && p == self.pattern {
                if changed {
                    self.clocks[t].index =
                        (self.beat / DIVISIONS[track.division as usize]).floor() as i64 + 1;
                }
                self.clocks[t].due = event_beat(track, self.clocks[t].index, self.swing);
            }
        }
    }
    pub fn step_extras(
        &mut self,
        p: usize,
        t: usize,
        s: usize,
        micro: f32,
        condition: u8,
        every: u8,
        phase: u8,
    ) {
        if p < PATTERNS && t < TRACKS && s < STEPS && micro.is_finite() {
            let step = &mut self.patterns[p].tracks[t].steps[s];
            step.micro = micro.clamp(-0.49, 0.49);
            step.condition = condition.min(7);
            step.every = every.clamp(1, 16);
            step.phase = phase % step.every;
        }
    }
    pub fn set_route(&mut self, index: usize, source: u8, voice: u8, param: u8, amount: f32) {
        if index < ROUTES
            && source < 9
            && voice < 6
            && (param as usize) < PARAMS
            && amount.is_finite()
        {
            self.routes[index] = Route {
                source,
                voice,
                param,
                amount: amount.clamp(-1., 1.),
            };
            self.refresh();
        }
    }
    pub fn set_macro(&mut self, index: usize, value: f32) {
        if index < 4 && value.is_finite() {
            self.macros[index] = value.clamp(0., 1.);
            self.global_macros[index] = self.macros[index];
            if self.scenes[self.pattern].enabled {
                self.scenes[self.pattern].macros[index] = self.macros[index];
            }
            self.refresh();
        }
    }
    pub fn sample_settings(
        &mut self,
        t: usize,
        gain: f32,
        tune: f32,
        start: f32,
        end: f32,
        decay: f32,
        replace: bool,
    ) {
        if t >= TRACKS
            || [gain, tune, start, end, decay]
                .iter()
                .any(|x| !x.is_finite())
        {
            return;
        }
        let s = &mut self.samples[t];
        s.gain = gain.clamp(0., 1.5);
        s.tune = tune.clamp(-36., 36.);
        s.start = start.clamp(0., 0.99);
        s.end = end.clamp(s.start + 0.001, 1.);
        s.decay = decay.clamp(0.005, 10.);
        s.replace = replace;
    }
    pub fn sample_data(&mut self, t: usize, rate: f32, pcm: Vec<f32>) {
        if t < TRACKS
            && (8000. ..=192000.).contains(&rate)
            && pcm.len() <= 960000
            && pcm.iter().all(|x| x.is_finite() && x.abs() <= 1.)
        {
            self.samples[t].pcm = pcm.into();
            self.samples[t].rate = rate;
            self.samples[t].stop();
        }
    }
    /// Move the live DSP state across an editor snapshot without allocating on audio.
    pub fn take_audio(&mut self, old: &mut Self) {
        let fx = self.fx_values();
        std::mem::swap(&mut self.voices, &mut old.voices);
        std::mem::swap(&mut self.fx, &mut old.fx);
        std::mem::swap(&mut self.resampler, &mut old.resampler);
        std::mem::swap(&mut self.rng, &mut old.rng);
        self.locks = old.locks;
        self.smooth_levels = old.smooth_levels;
        self.smooth_master = old.smooth_master;
        self.meters = old.meters;
        self.peak = old.peak;
        for t in 0..TRACKS {
            self.samples[t].inherit_playback(&old.samples[t]);
        }
        for (id, value) in fx.into_iter().enumerate() {
            self.set_fx_raw(id, value);
        }
        self.refresh();
    }
    pub fn inherit_clock(&mut self, old: &Self) {
        self.beat = old.beat;
        self.clock_valid = old.clock_valid;
        self.clocks = old.clocks;
        self.ratchets = old.ratchets;
        self.frame_count = old.frame_count;
        self.playing = old.playing;
        self.positions = old.positions;
        self.pending_switch = old.pending_switch;
        self.queued_pattern = old.queued_pattern;
        self.performance_velocity = old.performance_velocity;
        self.pressure = old.pressure;
        self.modwheel = old.modwheel;
        self.fill = old.fill;
        // Recompute event dates when an editor changes a division or step offset.
        for t in 0..TRACKS {
            let k = self.clocks[t].index;
            self.clocks[t].due = event_beat(&self.patterns[self.pattern].tracks[t], k, self.swing);
        }
    }
    pub fn seek_clock(&mut self, beat: f64) {
        self.beat = beat;
        self.clock_valid = false;
        self.ratchets = [Ratchet::default(); TRACKS];
    }
    pub fn render_at(&mut self, frames: usize, beat: f64, bpm: f64, gate: bool) -> &[f32] {
        if (self.beat - beat).abs() > 2. * bpm / (60. * self.sample_rate as f64) {
            self.seek_clock(beat);
        }
        self.beat = beat;
        self.bpm = bpm as f32;
        self.playing = gate;
        self.render_clocked(frames, true)
    }
    fn init_clocks(&mut self) {
        for t in 0..TRACKS {
            let tr = &self.patterns[self.pattern].tracks[t];
            let mut k = (self.beat / DIVISIONS[tr.division as usize] + 1e-8).floor() as i64;
            if k != 0 && event_beat(tr, k, self.swing) > self.beat + 1e-10 {
                k -= 1;
            }
            if event_beat(tr, k + 1, self.swing) <= self.beat + 1e-10 {
                k += 1;
            }
            let due = event_beat(tr, k, self.swing).max(self.beat);
            self.clocks[t] = TrackClock {
                index: k,
                due,
                previous: false,
                cycle_origin: k.div_euclid(tr.length as i64),
            };
        }
        self.clock_valid = true;
    }
    fn sequence_frame(&mut self) {
        if let Some((p, due)) = self.pending_switch {
            if self.beat + 1e-10 >= due {
                self.immediate_pattern(p);
            }
        }
        if !self.clock_valid {
            self.init_clocks();
        }
        for t in 0..TRACKS {
            if self.beat + 1e-10 < self.clocks[t].due {
                continue;
            }
            let k = self.clocks[t].index;
            let tr = &self.patterns[self.pattern].tracks[t];
            let position = k.rem_euclid(tr.length as i64) as usize;
            self.positions[t] = position as i32;
            let step = tr.steps[position];
            let cycle = k.div_euclid(tr.length as i64) - self.clocks[t].cycle_origin;
            let next = event_beat(tr, k + 1, self.swing);
            let due = self.clocks[t].due;
            let duration = ((next - due) * 60. * self.sample_rate as f64 / self.bpm as f64).max(1.);
            let muted = if self.scenes[self.pattern].enabled {
                self.scenes[self.pattern].muted[t]
            } else {
                self.muted[t]
            };
            let hit = step.velocity > 0.
                && !muted
                && condition(
                    step.condition,
                    step.every,
                    step.phase,
                    cycle,
                    self.fill,
                    self.clocks[t].previous,
                )
                && self.rng.unit() < step.probability;
            self.clocks[t].previous = hit;
            self.clocks[t].index = k + 1;
            self.clocks[t].due = next;
            self.ratchets[t] = Ratchet::default();
            if hit {
                self.locks[t.min(5)] = step.locks;
                self.refresh();
                let velocity = step.velocity
                    * (1. - self.velocity_depth + self.velocity_depth * self.performance_velocity);
                self.trigger(t, velocity, step.note as i32);
                if step.ratchet > 1 {
                    self.ratchets[t] = Ratchet {
                        remaining: step.ratchet - 1,
                        countdown: duration / step.ratchet as f64 + 1.,
                        interval: duration / step.ratchet as f64,
                        velocity,
                        note: step.note,
                    };
                }
            }
        }
        self.tick_count = (self.beat * 4.).max(0.) as u64 + 1;
    }
    pub fn hits(&self) -> &[Hit] {
        &self.hits[..self.hit_count]
    }
    pub fn clear_hits(&mut self) {
        self.hit_count = 0;
    }
    pub fn set_fx(&mut self, param: usize, value: f32) {
        self.set_fx_raw(param, value);
        if param < 15 && value.is_finite() {
            self.global_fx[param] = self.fx_values()[param];
        }
        if param < 15 && value.is_finite() && self.scenes[self.pattern].enabled {
            self.scenes[self.pattern].fx[param] = self.fx_values()[param];
        }
    }
    fn set_fx_raw(&mut self, param: usize, value: f32) {
        if !value.is_finite() {
            return;
        }
        match param {
            0 => self.fx.kind = value.clamp(0., 7.) as u8,
            1 => self.fx.mix = value.clamp(0., 1.),
            2 => self.fx.time = value.clamp(0., 0.743),
            3 => self.fx.feedback = value.clamp(0., 0.94),
            4 => self.fx.tone = value.clamp(0., 1.),
            5 => self.fx.drive = value.clamp(0., 1.),
            6 => self.master = value.clamp(0., 1.5),
            7 => self.fx.color = value.clamp(0., 1.),
            8 => self.fx.level = value.clamp(0., 1.),
            9 => self.fx.ring_wave = value.clamp(0., 5.) as u8,
            10 => self.fx.attack = value.clamp(0., 1.),
            11 => self.fx.release = value.clamp(0., 1.),
            12 => self.fx.threshold = value.clamp(0., 1.),
            13 => self.fx.ratio = value.clamp(1., 8.).round(),
            14 => self.fx.delay_range = value.clamp(0., 1.),
            _ => {}
        }
    }
}

fn modulated(id: usize, value: f32, amount: f32) -> f32 {
    let log = [
        PITCH,
        DECAY,
        ATTACK,
        PITCH_DECAY,
        FM_RATIO,
        CUTOFF,
        LFO_RATE,
        FM_RATIO2,
        OPEN_DECAY,
        CLICK_FREQ,
        NOISE_FREQ,
    ]
    .contains(&id);
    let value = if log {
        let low = MIN[id] + 0.001;
        let ratio = (MAX[id] + 0.001) / low;
        let norm = ((value + 0.001) / low).ln() / ratio.ln();
        low * ratio.powf((norm + amount).clamp(0., 1.)) - 0.001
    } else {
        value + amount * (MAX[id] - MIN[id])
    };
    constrain(id, value).unwrap_or(MIN[id])
}

thread_local! {static ENGINE:RefCell<Option<Engine>>=const{RefCell::new(None)};}
fn with<R: Default>(f: impl FnOnce(&mut Engine) -> R) -> R {
    ENGINE.with(|e| e.borrow_mut().as_mut().map(f).unwrap_or_default())
}
#[no_mangle]
pub extern "C" fn lxr_init(sr: f32) {
    ENGINE.with(|e| *e.borrow_mut() = Some(Engine::new(sr)));
}
#[no_mangle]
pub extern "C" fn lxr_param(v: u32, p: u32, x: f32) {
    with(|e| e.set_param(v as usize, p as usize, x));
}
#[no_mangle]
pub extern "C" fn lxr_get_param(v: u32, p: u32) -> f32 {
    with(|e| {
        e.params
            .get(v as usize)
            .and_then(|a| a.get(p as usize))
            .copied()
            .unwrap_or(0.)
    })
}
#[no_mangle]
pub extern "C" fn lxr_param_min(p: u32) -> f32 {
    MIN.get(p as usize).copied().unwrap_or(0.)
}
#[no_mangle]
pub extern "C" fn lxr_param_max(p: u32) -> f32 {
    MAX.get(p as usize).copied().unwrap_or(0.)
}
#[no_mangle]
pub extern "C" fn lxr_load_kit(i: u32) {
    with(|e| e.load_kit(i as usize));
}
#[no_mangle]
pub extern "C" fn lxr_morph(x: f32, target: u32) {
    with(|e| e.set_morph(x, target as usize));
}
#[no_mangle]
pub extern "C" fn lxr_tempo(bpm: f32, swing: f32) {
    with(|e| e.tempo(bpm, swing));
}
#[no_mangle]
pub extern "C" fn lxr_play() {
    with(Engine::play);
}
#[no_mangle]
pub extern "C" fn lxr_stop() {
    with(Engine::stop);
}
#[no_mangle]
pub extern "C" fn lxr_panic() {
    with(Engine::panic);
}
#[no_mangle]
pub extern "C" fn lxr_trigger(t: u32, velocity: f32, note: i32) {
    with(|e| e.trigger(t as usize, velocity, note));
}
#[no_mangle]
pub extern "C" fn lxr_pattern(p: u32) {
    with(|e| e.select_pattern(p as usize));
}
#[no_mangle]
pub extern "C" fn lxr_get_pattern() -> u32 {
    with(|e| e.pattern as u32)
}
#[no_mangle]
pub extern "C" fn lxr_set_step(p: u32, t: u32, s: u32, v: f32, prob: f32, note: i32, ratchet: u32) {
    with(|e| e.set_step(p as usize, t as usize, s as usize, v, prob, note, ratchet));
}
#[no_mangle]
pub extern "C" fn lxr_set_lock(p: u32, t: u32, s: u32, slot: u32, param: u32, value: f32) {
    with(|e| {
        e.set_lock(
            p as usize,
            t as usize,
            s as usize,
            slot as usize,
            param as usize,
            value,
        )
    });
}
#[no_mangle]
pub extern "C" fn lxr_get_step(p: u32, t: u32, s: u32, field: u32) -> f32 {
    with(|e| {
        if p as usize >= PATTERNS || t as usize >= TRACKS || s as usize >= STEPS {
            return 0.;
        }
        let a = e.patterns[p as usize].tracks[t as usize].steps[s as usize];
        match field {
            0 => a.velocity,
            1 => a.probability,
            2 => a.note as f32,
            3 => a.ratchet as f32,
            4 => a.locks[0].param as f32,
            5 => a.locks[0].value,
            6 => a.locks[1].param as f32,
            7 => a.locks[1].value,
            8 => a.micro,
            9 => a.condition as f32,
            10 => a.every as f32,
            11 => a.phase as f32,
            _ => 0.,
        }
    })
}
#[no_mangle]
pub extern "C" fn lxr_length(p: u32, t: u32, n: u32) {
    with(|e| e.set_length(p as usize, t as usize, n as usize));
}
#[no_mangle]
pub extern "C" fn lxr_get_length(p: u32, t: u32) -> u32 {
    with(|e| {
        e.patterns
            .get(p as usize)
            .and_then(|p| p.tracks.get(t as usize))
            .map(|t| t.length as u32)
            .unwrap_or(16)
    })
}
#[no_mangle]
pub extern "C" fn lxr_generate(p: u32, t: u32, k: u32, n: u32, r: u32) {
    with(|e| e.generate(p as usize, t as usize, k as usize, n as usize, r as usize));
}
#[no_mangle]
pub extern "C" fn lxr_level(t: u32, x: f32) {
    with(|e| {
        if (t as usize) < TRACKS && x.is_finite() {
            e.set_mix(t as usize, 0, x);
        }
    });
}
#[no_mangle]
pub extern "C" fn lxr_mute(t: u32, x: u32) {
    with(|e| {
        if (t as usize) < TRACKS {
            e.set_mix(t as usize, 1, x as f32);
        }
    });
}
#[no_mangle]
pub extern "C" fn lxr_solo(t: u32, x: u32) {
    with(|e| {
        if (t as usize) < TRACKS {
            e.set_mix(t as usize, 2, x as f32);
        }
    });
}
#[no_mangle]
pub extern "C" fn lxr_fx(p: u32, x: f32) {
    with(|e| e.set_fx(p as usize, x));
}
#[no_mangle]
pub extern "C" fn lxr_process(n: u32) -> u32 {
    with(|e| {
        e.render(n as usize);
        n.min(MAX_FRAMES as u32)
    })
}
#[no_mangle]
pub extern "C" fn lxr_buffer() -> *const f32 {
    with(|e| e.output.as_ptr() as usize) as *const f32
}
#[no_mangle]
pub extern "C" fn lxr_position(t: u32) -> i32 {
    with(|e| e.positions.get(t as usize).copied().unwrap_or(-1))
}
#[no_mangle]
pub extern "C" fn lxr_meter(t: u32) -> f32 {
    with(|e| e.meters.get(t as usize).copied().unwrap_or(0.))
}
#[no_mangle]
pub extern "C" fn lxr_peak(c: u32) -> f32 {
    with(|e| e.peak.get(c as usize).copied().unwrap_or(0.))
}

#[no_mangle]
pub extern "C" fn lxr_track_get(p: u32, t: u32, field: u32) -> f32 {
    with(|e| {
        e.patterns
            .get(p as usize)
            .and_then(|p| p.tracks.get(t as usize))
            .map(|t| match field {
                0 => t.division as f32,
                1 => t.delay,
                2 => t.groove as f32,
                3 => t.groove_amount,
                _ => 0.,
            })
            .unwrap_or(0.)
    })
}
#[no_mangle]
pub extern "C" fn lxr_track_settings(
    p: u32,
    t: u32,
    division: u32,
    delay: f32,
    groove: u32,
    amount: f32,
) {
    with(|e| {
        e.track_settings(
            p as usize,
            t as usize,
            division as u8,
            delay,
            groove as u8,
            amount,
        )
    });
}
#[no_mangle]
pub extern "C" fn lxr_step_extras(
    p: u32,
    t: u32,
    s: u32,
    micro: f32,
    condition: u32,
    every: u32,
    phase: u32,
) {
    with(|e| {
        e.step_extras(
            p as usize,
            t as usize,
            s as usize,
            micro,
            condition as u8,
            every as u8,
            phase as u8,
        )
    });
}
#[no_mangle]
pub extern "C" fn lxr_route(index: u32, source: u32, voice: u32, param: u32, amount: f32) {
    with(|e| {
        e.set_route(
            index as usize,
            source as u8,
            voice as u8,
            param as u8,
            amount,
        )
    });
}
#[no_mangle]
pub extern "C" fn lxr_macro(index: u32, value: f32) {
    with(|e| e.set_macro(index as usize, value));
}
#[no_mangle]
pub extern "C" fn lxr_performance(kind: u32, value: f32) {
    if !value.is_finite() {
        return;
    }
    with(|e| match kind {
        0 => e.switch_quant = value.clamp(0., 2.) as u8,
        1 => e.velocity_depth = value.clamp(0., 1.),
        2 => e.fill = value >= 0.5,
        3 => e.pressure = value.clamp(0., 1.),
        4 => e.modwheel = value.clamp(0., 1.),
        5 | 6 => e.lfo_periods[(kind - 5) as usize] = value.clamp(0.0625, 32.),
        _ => {}
    });
}
#[no_mangle]
pub extern "C" fn lxr_scene_capture(p: u32) {
    with(|e| e.capture_scene(p as usize, true));
}
#[no_mangle]
pub extern "C" fn lxr_scene_enable(p: u32, enabled: u32) {
    with(|e| {
        if (p as usize) < PATTERNS {
            e.scenes[p as usize].enabled = enabled != 0;
            if e.pattern == p as usize {
                e.immediate_pattern(p as usize);
            }
        }
    });
}
#[no_mangle]
pub extern "C" fn lxr_scene_param(p: u32, v: u32, id: u32, value: f32) {
    with(|e| {
        if (p as usize) < PATTERNS && (v as usize) < VOICES {
            if let Some(x) = constrain(id as usize, value) {
                e.scenes[p as usize].params[v as usize][id as usize] = x;
            }
        }
    });
}
#[no_mangle]
pub extern "C" fn lxr_scene_mix(p: u32, t: u32, kind: u32, value: f32) {
    with(|e| {
        if (p as usize) < PATTERNS && (t as usize) < TRACKS && value.is_finite() {
            let s = &mut e.scenes[p as usize];
            match kind {
                0 => s.levels[t as usize] = value.clamp(0., 1.25),
                1 => s.muted[t as usize] = value >= 0.5,
                2 => s.solo[t as usize] = value >= 0.5,
                _ => {}
            }
        }
    });
}
#[no_mangle]
pub extern "C" fn lxr_scene_fx(p: u32, id: u32, value: f32) {
    with(|e| {
        if (p as usize) < PATTERNS && id < 15 && value.is_finite() {
            e.scenes[p as usize].fx[id as usize] = value;
        }
    });
}
#[no_mangle]
pub extern "C" fn lxr_scene_misc(p: u32, id: u32, value: f32) {
    with(|e| {
        if (p as usize) < PATTERNS && value.is_finite() {
            let s = &mut e.scenes[p as usize];
            match id {
                0..=3 => s.macros[id as usize] = value.clamp(0., 1.),
                4 => s.morph = value.clamp(0., 1.),
                5 => s.morph_target = value.clamp(0., 4.) as usize,
                _ => {}
            }
        }
    });
}
#[no_mangle]
pub extern "C" fn lxr_sample_settings(
    t: u32,
    gain: f32,
    tune: f32,
    start: f32,
    end: f32,
    decay: f32,
    replace: u32,
) {
    with(|e| e.sample_settings(t as usize, gain, tune, start, end, decay, replace != 0));
}
thread_local! {static SAMPLE_UPLOAD:RefCell<Vec<f32>>=const{RefCell::new(Vec::new())};}
#[no_mangle]
pub extern "C" fn lxr_sample_alloc(count: u32) -> *mut f32 {
    if count > 960000 {
        return std::ptr::null_mut();
    }
    SAMPLE_UPLOAD.with(|s| {
        let mut s = s.borrow_mut();
        s.resize(count as usize, 0.);
        s.as_mut_ptr()
    })
}
#[no_mangle]
pub extern "C" fn lxr_sample_commit(t: u32, rate: f32) {
    let data = SAMPLE_UPLOAD.with(|s| std::mem::take(&mut *s.borrow_mut()));
    with(|e| e.sample_data(t as usize, rate, data));
}
#[no_mangle]
pub extern "C" fn lxr_event_count() -> u32 {
    with(|e| e.hits().len() as u32)
}
#[no_mangle]
pub extern "C" fn lxr_event(index: u32, field: u32) -> f64 {
    with(|e| {
        e.hits()
            .get(index as usize)
            .map(|h| match field {
                0 => h.frame as f64,
                1 => h.track as f64,
                2 => h.note as f64,
                3 => h.velocity as f64,
                _ => 0.,
            })
            .unwrap_or(0.)
    })
}
#[no_mangle]
pub extern "C" fn lxr_events_clear() {
    with(|e| e.clear_hits());
}

#[cfg(test)]
mod engine_tests {
    use super::*;
    #[test]
    fn manual_edit_and_stop_release_temporary_parameter_locks() {
        let mut e = Engine::new(44100.);
        e.set_lock(0, 0, 0, 0, PITCH, 300.);
        e.play();
        e.render(1);
        assert_eq!(e.voices[0].p[PITCH], 300.);
        e.set_param(0, PITCH, 70.);
        assert_eq!(e.voices[0].p[PITCH], 70.);
        e.stop();
        e.play();
        e.render(1);
        assert_eq!(e.voices[0].p[PITCH], 300.);
        e.stop();
        assert_eq!(e.voices[0].p[PITCH], 70.);
        assert_eq!(e.patterns[0].tracks[0].steps[0].locks[0].value, 300.);
    }
}
