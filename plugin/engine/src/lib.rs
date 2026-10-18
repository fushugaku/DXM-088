//! Per-instance instrument, host clock and MIDI gate. The audio path allocates nothing.
use lxr02::{params::*, sequencer::PATTERNS, Engine, MAX_FRAMES, OUTPUT_CHANNELS};
use serde_json::{json, Value};
use std::{
    ffi::{c_char, c_void},
    slice,
};

mod performance;
pub const PARAMETERS: usize = 292;
pub const DEFAULT_DRUM_NOTES: [u8; 7] = [36, 45, 47, 38, 49, 42, 46];
const FX_MIN: [f32; 15] = [0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 1., 0.];
const FX_MAX: [f32; 15] = [
    7., 1., 0.743, 0.94, 1., 1., 1.5, 1., 1., 5., 1., 1., 1., 8., 1.,
];
const NAMES: [&str; 39] = [
    "Pitch",
    "Waveform",
    "Decay",
    "Attack",
    "Slope",
    "Pitch sweep",
    "Pitch decay",
    "FM ratio",
    "FM amount",
    "FM waveform",
    "Noise mix",
    "Click level",
    "Cutoff",
    "Resonance",
    "Filter type",
    "Drive",
    "Distortion",
    "Reduction",
    "Pan",
    "Voice level",
    "LFO rate",
    "LFO depth",
    "LFO target",
    "LFO wave",
    "FM ratio 2",
    "FM gain 2",
    "Pulse width",
    "Transient",
    "Repeat",
    "Open decay",
    "Velocity target",
    "Velocity amount",
    "OSC mode",
    "LFO retrigger",
    "Transient rate",
    "Noise color",
    "Filter drive",
    "Pitch slope",
    "FM wave 2",
];
const FX_NAMES: [&str; 15] = [
    "FX type",
    "FX mix",
    "FX time",
    "FX feedback",
    "FX tone",
    "FX drive",
    "Master",
    "FX color",
    "FX output",
    "Ring waveform",
    "Comp attack",
    "Comp release",
    "Comp threshold",
    "Comp ratio",
    "Delay range",
];

pub struct Instrument {
    pub engine: Engine,
    held: [u8; 2048],
    order: [u64; 2048],
    serial: u64,
    pub active_note: Option<u8>,
    pub base_note: u8,
    pub preview: bool,
    /// 0: held notes select patterns; 1: one-shot drum notes.
    pub midi_mode: u8,
    /// 0: main stereo with shared FX; 1: seven individual pre-master-FX pairs.
    pub output_mode: u8,
    pub drum_notes: [u8; 7],
    beat: f64,
    bpm: f64,
    host_running: bool,
    expected: Option<f64>,
    last_tick: Option<i64>,
    name: String,
    kit: usize,
    kit_preset: Option<Value>,
    pattern_memory: bool,
    velocities: [u8; 2048],
    pressures: [u8; 16],
    wheels: [u8; 16],
    macro_names: [String; 4],
    sample_names: [String; 7],
    mutation: Value,
}
impl Instrument {
    pub fn new(rate: f32) -> Self {
        Self {
            engine: Engine::new(rate),
            held: [0; 2048],
            order: [0; 2048],
            serial: 0,
            active_note: None,
            base_note: 0,
            preview: false,
            midi_mode: 0,
            output_mode: 0,
            drum_notes: DEFAULT_DRUM_NOTES,
            beat: 0.,
            bpm: 128.,
            host_running: false,
            expected: None,
            last_tick: None,
            name: "Session 01".into(),
            kit: 0,
            kit_preset: None,
            pattern_memory: false,
            velocities: [127; 2048],
            pressures: [0; 16],
            wheels: [0; 16],
            macro_names: std::array::from_fn(|i| format!("Macro {}", i + 1)),
            sample_names: std::array::from_fn(|_| String::new()),
            mutation: json!({"depth":0.25,"locked":[0,2,3,5,6,18,19,29]}),
        }
    }
    pub fn copy_project(&self, rate: f32) -> Self {
        let mut n = Self::new(rate);
        n.engine.params = self.engine.params;
        n.engine
            .patterns
            .copy_from_slice(self.engine.patterns.as_slice());
        for p in 234..PARAMETERS {
            n.set(p, self.get(p));
        }
        n.engine.tempo(self.engine.bpm, self.engine.swing);
        n.engine.immediate_pattern(self.engine.pattern);
        n.name = self.name.clone();
        n.kit = self.kit;
        n.kit_preset = self.kit_preset.clone();
        n.pattern_memory = self.pattern_memory;
        n.engine
            .scenes
            .copy_from_slice(self.engine.scenes.as_slice());
        n.engine.samples = self.engine.samples.clone();
        n.engine.routes = self.engine.routes;
        n.engine.lfo_periods = self.engine.lfo_periods;
        n.macro_names = self.macro_names.clone();
        n.sample_names = self.sample_names.clone();
        n.mutation = self.mutation.clone();
        n.engine.immediate_pattern(self.engine.pattern);
        n
    }
    /// An editor snapshot replaces project data, while held host notes keep playing.
    pub fn inherit_transport(&mut self, old: &Self) {
        self.beat = old.beat;
        self.bpm = old.bpm;
        self.host_running = old.host_running;
        self.expected = old.expected;
        self.last_tick = None;
        if self.base_note == old.base_note
            && self.midi_mode == old.midi_mode
            && self.drum_notes == old.drum_notes
        {
            self.held = old.held;
            self.order = old.order;
            self.velocities = old.velocities;
            self.pressures = old.pressures;
            self.wheels = old.wheels;
            self.serial = old.serial;
            self.active_note = old.active_note;
            self.preview = old.preview;
            if self.midi_mode == 0 && self.active_note.is_some() {
                self.engine.immediate_pattern(old.engine.pattern);
            }
            if self.active_note.is_some() || self.preview {
                self.engine.inherit_clock(&old.engine);
            }
        }
    }
    fn refresh_gate(&mut self) {
        let current_index = self
            .held
            .iter()
            .enumerate()
            .filter(|(_, count)| **count > 0)
            .max_by_key(|(i, _)| self.order[*i])
            .map(|(i, _)| i);
        if let Some(i) = current_index {
            self.engine.performance_velocity = self.velocities[i] as f32 / 127.;
            self.engine.pressure = self.pressures[i / 128] as f32 / 127.;
            self.engine.modwheel = self.wheels[i / 128] as f32 / 127.;
        }
        let current = current_index.map(|i| (i % 128) as u8);
        if self.midi_mode == 1 {
            // Percussion tails are independent of note length. Only preview runs
            // the internal sequencer in this mode.
            self.active_note = current;
            return;
        }
        if current.is_some() {
            self.preview = false;
        }
        if current != self.active_note {
            let was_idle = self.active_note.is_none();
            self.active_note = current;
            self.last_tick = None;
            if let Some(note) = current {
                if was_idle {
                    if !self.host_running {
                        self.beat = 0.;
                    }
                    self.engine
                        .immediate_pattern(note.wrapping_sub(self.base_note) as usize % PATTERNS);
                    self.engine.seek_clock(self.beat);
                    self.engine.play();
                } else {
                    self.engine
                        .select_pattern(note.wrapping_sub(self.base_note) as usize % PATTERNS);
                }
            } else {
                self.engine.stop();
            }
        }
    }
    pub fn midi(&mut self, status: u8, key: u8, value: u8) {
        let channel = (status & 15) as usize;
        let key = key.min(127);
        let at = channel * 128 + key as usize;
        match status & 0xf0 {
            0x90 if value > 0 => {
                if self.midi_mode == 1 && !self.drum_notes.contains(&key) {
                    return;
                }
                self.serial = self.serial.wrapping_add(1);
                self.held[at] = self.held[at].saturating_add(1);
                self.order[at] = self.serial;
                self.velocities[at] = value;
                self.refresh_gate();
                if self.midi_mode == 1 {
                    for t in 0..7 {
                        if self.drum_notes[t] == key {
                            self.engine.trigger(t, value as f32 / 127., 0);
                        }
                    }
                }
            }
            0x80 | 0x90 => {
                self.held[at] = self.held[at].saturating_sub(1);
                self.refresh_gate();
            }
            0xb0 if key == 120 || key == 123 => {
                self.held[channel * 128..channel * 128 + 128].fill(0);
                self.refresh_gate();
                if key == 120 || self.midi_mode == 1 {
                    self.preview = false;
                    self.engine.panic();
                    self.last_tick = None;
                }
            }
            0xd0 => {
                self.pressures[channel] = key;
                self.refresh_gate();
            }
            0xa0 if Some(key) == self.active_note => {
                self.pressures[channel] = value;
                self.refresh_gate();
            }
            0xb0 if key == 1 => {
                self.wheels[channel] = value;
                self.refresh_gate();
            }
            0xb0 if key == 64 => self.engine.fill = value >= 64,
            _ => {}
        }
    }
    pub fn all_off(&mut self) {
        self.held.fill(0);
        self.active_note = None;
        self.preview = false;
        self.last_tick = None;
        self.engine.stop();
    }
    pub fn transport(&mut self, bpm: f64, beat: f64, running: bool) {
        if bpm.is_finite() && bpm > 0. {
            self.bpm = bpm.clamp(1., 1000.);
            self.engine.bpm = self.bpm as f32;
        }
        if self.host_running && !running {
            self.all_off();
        }
        if running && beat.is_finite() {
            let tolerance = 2. * self.bpm / (60. * self.engine.sample_rate as f64);
            if !self.host_running || self.expected.is_some_and(|p| (p - beat).abs() > tolerance) {
                self.last_tick = None;
                self.engine.seek_clock(beat);
            }
            self.beat = beat;
        }
        self.host_running = running;
    }
    pub fn render(&mut self, out: &mut [f32]) {
        for chunk in out.chunks_mut(MAX_FRAMES * 2) {
            let n = chunk.len() / 2;
            let active = self.sequencer_active();
            chunk.copy_from_slice(self.engine.render_at(n, self.beat, self.bpm, active));
            if self.output_mode == 1 {
                chunk.fill(0.);
            }
            self.advance(n);
        }
    }
    pub fn sequencer_active(&self) -> bool {
        (self.midi_mode == 0 && self.active_note.is_some()) || self.preview
    }
    fn advance(&mut self, frames: usize) {
        if self.host_running || self.sequencer_active() {
            self.beat += frames as f64 * self.bpm / (60. * self.engine.sample_rate as f64);
        }
        self.expected = if self.host_running {
            Some(self.beat)
        } else {
            None
        };
    }
    /// Sixteen interleaved channels, rendered once with a shared sample clock.
    pub fn render_multi(&mut self, out: &mut [f32]) {
        for chunk in out.chunks_mut(MAX_FRAMES * OUTPUT_CHANNELS) {
            let n = chunk.len() / OUTPUT_CHANNELS;
            let active = self.sequencer_active();
            let main = self.engine.render_at(n, self.beat, self.bpm, active);
            for (frame, pair) in chunk
                .chunks_exact_mut(OUTPUT_CHANNELS)
                .zip(main.chunks_exact(2))
            {
                frame.fill(0.);
                if self.output_mode == 0 {
                    frame[..2].copy_from_slice(pair);
                }
            }
            if self.output_mode == 1 {
                for (frame, stem) in chunk
                    .chunks_exact_mut(OUTPUT_CHANNELS)
                    .zip(self.engine.stems(n).chunks_exact(14))
                {
                    frame[2..].copy_from_slice(stem);
                }
            }
            self.advance(n);
        }
    }
    pub fn get(&self, p: usize) -> f32 {
        let e = &self.engine;
        match p {
            0..234 => e.params[p / 39][p % 39],
            234..249 => e.global_fx[p - 234],
            249..256 => e.levels[p - 249],
            256..263 => e.muted[p - 256] as u8 as f32,
            263..270 => e.solo[p - 263] as u8 as f32,
            270 => e.swing,
            271 => e.global_morph[0],
            272 => e.global_morph[1],
            273 => self.base_note as f32,
            274..278 => e.global_macros[p - 274],
            278 => e.velocity_depth,
            279 => e.fill as u8 as f32,
            280 => e.switch_quant as f32,
            281..283 => e.lfo_periods[p - 281],
            283 => self.midi_mode as f32,
            284 => self.output_mode as f32,
            285..292 => self.drum_notes[p - 285] as f32,
            _ => 0.,
        }
    }
    pub fn set(&mut self, p: usize, v: f32) {
        if !v.is_finite() {
            return;
        }
        match p {
            0..234 => self.engine.set_param(p / 39, p % 39, v),
            234..249 => self.engine.set_fx(p - 234, v),
            249..256 => self.engine.set_mix(p - 249, 0, v),
            256..263 => self.engine.set_mix(p - 256, 1, v),
            263..270 => self.engine.set_mix(p - 263, 2, v),
            270 => self.engine.swing = v.clamp(0., 0.5),
            271 => self.engine.set_morph(v, self.engine.morph_target),
            272 => self
                .engine
                .set_morph(self.engine.morph, v.round().clamp(0., 4.) as usize),
            273 => {
                let b = v.round().clamp(0., 120.) as u8;
                if self.base_note != b {
                    self.all_off();
                    self.base_note = b;
                }
            }
            274..278 => self.engine.set_macro(p - 274, v),
            278 => self.engine.velocity_depth = v.clamp(0., 1.),
            279 => self.engine.fill = v >= 0.5,
            280 => self.engine.switch_quant = v.round().clamp(0., 2.) as u8,
            281..283 => self.engine.lfo_periods[p - 281] = v.clamp(0.0625, 32.),
            283 => {
                let mode = v.round().clamp(0., 1.) as u8;
                if self.midi_mode != mode {
                    self.all_off();
                    self.engine.panic();
                    self.midi_mode = mode;
                }
            }
            284 => {
                self.output_mode = v.round().clamp(0., 1.) as u8;
                self.engine.capture_stems = self.output_mode == 1;
            }
            285..292 => {
                let note = v.round().clamp(0., 127.) as u8;
                if self.drum_notes[p - 285] != note {
                    self.all_off();
                    self.engine.panic();
                    self.drum_notes[p - 285] = note;
                }
            }
            _ => {}
        }
    }
    pub fn command(&mut self, op: u32, a: [f64; 8]) {
        let u = |i: usize| a[i].max(0.) as usize;
        let f = |i: usize| a[i] as f32;
        match op {
            0 => self.engine.set_param(u(0), u(1), f(2)),
            1 => self.engine.tempo(f(0), f(1)),
            2 => {
                if self.midi_mode == 1 || self.active_note.is_none() {
                    self.engine.immediate_pattern(u(0));
                }
            }
            3 => self.engine.set_length(u(0), u(1), u(2)),
            4 => self
                .engine
                .set_step(u(0), u(1), u(2), f(3), f(4), a[5] as i32, u(6) as u32),
            5 => self.engine.set_lock(u(0), u(1), u(2), u(3), u(4), f(5)),
            6 => self.set(249 + u(0).min(6), f(1)),
            7 => self.set(256 + u(0).min(6), f(1)),
            8 => self.set(263 + u(0).min(6), f(1)),
            9 => self.engine.set_fx(u(0), f(1)),
            10 => self.engine.set_morph(f(0), u(1)),
            11 => {
                self.kit = u(0).min(4);
                self.kit_preset = None;
                self.engine.load_kit(self.kit);
            }
            12 => self.engine.trigger(u(0), f(1), a[2] as i32),
            13 => {
                self.all_off();
                self.engine.panic();
            }
            14 => self.all_off(),
            15 => {
                if self.midi_mode == 1 || self.active_note.is_none() {
                    self.preview = true;
                    if !self.host_running {
                        self.beat = 0.;
                    }
                    self.engine.seek_clock(self.beat);
                    self.engine.play();
                }
            }
            16 => self.engine.generate(u(0), u(1), u(2), u(3), u(4)),
            17 => self
                .engine
                .track_settings(u(0), u(1), u(2) as u8, f(3), u(4) as u8, f(5)),
            18 => {
                self.engine
                    .step_extras(u(0), u(1), u(2), f(3), u(4) as u8, u(5) as u8, u(6) as u8)
            }
            19 => self
                .engine
                .set_route(u(0), u(1) as u8, u(2) as u8, u(3) as u8, f(4)),
            20 => self.set(274 + u(0).min(3), f(1)),
            21 => match u(0) {
                0 => self.set(280, f(1)),
                1 => self.set(278, f(1)),
                2 => self.set(279, f(1)),
                3 => self.engine.pressure = f(1).clamp(0., 1.),
                4 => self.engine.modwheel = f(1).clamp(0., 1.),
                5 | 6 => self.set(281 + u(0) - 5, f(1)),
                _ => {}
            },
            22 => self.engine.capture_scene(u(0), true),
            23 => {
                if u(0) < PATTERNS {
                    self.engine.scenes[u(0)].enabled = u(1) != 0;
                    self.engine.immediate_pattern(self.engine.pattern);
                }
            }
            24 => self
                .engine
                .sample_settings(u(0), f(1), f(2), f(3), f(4), f(5), u(6) != 0),
            25..=28 if u(0) < PATTERNS => {
                let p = u(0);
                let scene = &mut self.engine.scenes[p];
                match op {
                    25 if u(1) < 6 && u(2) < 39 => {
                        scene.params[u(1)][u(2)] =
                            lxr02::params::constrain(u(2), f(3)).unwrap_or(scene.params[u(1)][u(2)])
                    }
                    26 if u(1) < 7 => match u(2) {
                        0 => scene.levels[u(1)] = f(3).clamp(0., 1.25),
                        1 => scene.muted[u(1)] = f(3) >= 0.5,
                        2 => scene.solo[u(1)] = f(3) >= 0.5,
                        _ => {}
                    },
                    27 if u(1) < 15 => {
                        let (min, max, _) = range(234 + u(1));
                        scene.fx[u(1)] = f(2).clamp(min, max);
                    }
                    28 => match u(1) {
                        0..=3 => scene.macros[u(1)] = f(2).clamp(0., 1.),
                        4 => scene.morph = f(2).clamp(0., 1.),
                        5 => scene.morph_target = u(2).min(4),
                        _ => {}
                    },
                    _ => {}
                }
                if p == self.engine.pattern {
                    if op == 25 && u(1) < 6 && u(2) < 39 {
                        self.engine.set_param(u(1), u(2), f(3));
                    } else {
                        self.engine.refresh_scene();
                    }
                }
            }
            _ => {}
        }
    }
    pub fn project(&self) -> Value {
        let e = &self.engine;
        let patterns:Vec<_>=e.patterns.iter().map(|p|json!({"lengths":p.tracks.map(|t|t.length),"divisions":p.tracks.map(|t|t.division),"delays":p.tracks.map(|t|t.delay),"grooves":p.tracks.map(|t|t.groove),"grooveAmounts":p.tracks.map(|t|t.groove_amount),"tracks":p.tracks.iter().map(|t|t.steps.iter().map(|s|json!({"v":s.velocity,"prob":s.probability,"note":s.note,"ratchet":s.ratchet,"micro":s.micro,"condition":s.condition,"every":s.every,"phase":s.phase,"locks":s.locks.map(|l|json!([l.param,l.value]))})).collect::<Vec<_>>()).collect::<Vec<_>>()})).collect();
        let mut j = json!({"format":"lxr02-web","version":4,"name":self.name,"kit":self.kit,"bpm":e.bpm,"swing":e.swing,"pattern":e.pattern,"params":e.params.map(|p|p.to_vec()),"patterns":patterns,"levels":e.levels,"muted":e.muted,"solo":e.solo,"fx":(234..249).map(|p|self.get(p)).collect::<Vec<_>>(),"morph":e.global_morph[0],"morphTarget":e.global_morph[1],"plugin":{"version":2,"baseNote":self.base_note,"nativeFileDrag":cfg!(target_os="macos")}});
        j["plugin"] = json!({"version":3,"baseNote":self.base_note,"midiMode":self.midi_mode,"outputMode":self.output_mode,"drumNotes":self.drum_notes,"nativeFileDrag":cfg!(target_os="macos")});
        j["kitPreset"] = self.kit_preset.clone().unwrap_or(Value::Null);
        j["plugin"]["patternMemory"] = json!(self.pattern_memory);
        self.extensions(&mut j);
        j
    }
    pub fn load(&mut self, j: &Value) -> Option<()> {
        if j["format"] != "lxr02-web" || ![2, 3, 4].contains(&j["version"].as_u64()?) {
            return None;
        }
        // Parse and validate into a separate instance: a malformed state never partially mutates a project.
        let mut n = Self::new(self.engine.sample_rate);
        n.name = j["name"]
            .as_str()
            .unwrap_or("Session 01")
            .chars()
            .take(60)
            .collect();
        n.kit = j["kit"].as_u64()?.min(4) as usize;
        n.kit_preset = kit_preset(&j["kitPreset"]);
        n.pattern_memory = j["plugin"]["patternMemory"].as_bool().unwrap_or(false);
        let num = |v: &Value, min: f64, max: f64| -> Option<f32> {
            let x = v.as_f64()?;
            (x.is_finite() && x >= min - 0.00001 && x <= max + 0.00001)
                .then_some(x.clamp(min, max) as f32)
        };
        let count = if j["version"] == 4 { PATTERNS } else { 8 };
        if j["params"].as_array()?.len() != 6 || j["patterns"].as_array()?.len() != count {
            return None;
        }
        for v in 0..6 {
            if j["params"][v].as_array()?.len() != 39 {
                return None;
            }
            for p in 0..39 {
                n.engine
                    .set_param(v, p, num(&j["params"][v][p], MIN[p] as f64, MAX[p] as f64)?);
            }
        }
        for p in 0..count {
            let pattern = &j["patterns"][p];
            if pattern["lengths"].as_array()?.len() != 7 || pattern["tracks"].as_array()?.len() != 7
            {
                return None;
            }
            for t in 0..7 {
                n.engine
                    .set_length(p, t, num(&pattern["lengths"][t], 1., 64.)? as usize);
                if pattern["tracks"][t].as_array()?.len() != 64 {
                    return None;
                }
                for s in 0..64 {
                    let x = &pattern["tracks"][t][s];
                    n.engine.set_step(
                        p,
                        t,
                        s,
                        num(&x["v"], 0., 1.)?,
                        num(&x["prob"], 0., 1.)?,
                        num(&x["note"], -36., 36.)? as i32,
                        num(&x["ratchet"], 1., 4.)? as u32,
                    );
                    if x["locks"].as_array()?.len() != 2 {
                        return None;
                    }
                    for slot in 0..2 {
                        let lock = &x["locks"][slot];
                        if lock.as_array()?.len() != 2 {
                            return None;
                        }
                        let id = lock[0].as_u64()? as usize;
                        if id == 255 {
                            continue;
                        }
                        if id >= PARAMS {
                            return None;
                        }
                        n.engine.set_lock(
                            p,
                            t,
                            s,
                            slot,
                            id,
                            num(&lock[1], MIN[id] as f64, MAX[id] as f64)?,
                        );
                    }
                }
            }
        }
        for (key, start, count) in [
            ("fx", 234, 15),
            ("levels", 249, 7),
            ("muted", 256, 7),
            ("solo", 263, 7),
        ] {
            if j[key].as_array()?.len() != count {
                return None;
            }
            for i in 0..count {
                let value = if start >= 256 {
                    j[key][i].as_bool()? as u8 as f32
                } else {
                    let (min, max, _) = range(start + i);
                    num(&j[key][i], min as f64, max as f64)?
                };
                n.set(start + i, value);
            }
        }
        n.engine
            .tempo(num(&j["bpm"], 1., 1000.)?, num(&j["swing"], 0., 0.5)?);
        n.engine
            .immediate_pattern(num(&j["pattern"], 0., (count - 1) as f64)? as usize);
        n.engine.set_morph(
            num(&j["morph"], 0., 1.)?,
            num(&j["morphTarget"], 0., 4.)? as usize,
        );
        let default_origin = if j["version"] == 4 { 0. } else { 24. };
        n.set(
            273,
            j["plugin"]["baseNote"].as_f64().unwrap_or(default_origin) as f32,
        );
        for (key, id) in [("midiMode", 283), ("outputMode", 284)] {
            if let Some(value) = j["plugin"].get(key) {
                n.set(id, num(value, 0., 1.)?);
            }
        }
        if let Some(notes) = j["plugin"].get("drumNotes") {
            if notes.as_array()?.len() != 7 {
                return None;
            }
            for t in 0..7 {
                n.set(285 + t, num(&notes[t], 0., 127.)?);
            }
        }
        n.load_extensions(j)?;
        // Legacy MIDI clips retain their origin and original eight patterns.
        for p in count..PATTERNS {
            n.engine.patterns[p] = n.engine.patterns[p % count];
            n.engine.scenes[p] = n.engine.scenes[p % count];
        }
        for t in 0..7 {
            if n.engine.samples[t].pcm.as_ref() == self.engine.samples[t].pcm.as_ref() {
                n.engine.samples[t].pcm = self.engine.samples[t].pcm.clone();
            }
        }
        *self = n;
        Some(())
    }
}
fn kit_preset(value: &Value) -> Option<Value> {
    let id = value["id"].as_str()?;
    let name = value["name"].as_str()?;
    if id.is_empty()
        || id.len() > 64
        || !id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
        || name.trim().is_empty()
    {
        return None;
    }
    Some(json!({"id":id,"name":name.chars().take(60).collect::<String>()}))
}
pub fn range(p: usize) -> (f32, f32, bool) {
    match p {
        0..234 => (MIN[p % 39], MAX[p % 39], DISCRETE.contains(&(p % 39))),
        234..249 => (
            FX_MIN[p - 234],
            FX_MAX[p - 234],
            [234, 243, 247].contains(&p),
        ),
        249..256 => (0., 1.25, false),
        256..270 => (0., 1., true),
        270 => (0., 0.5, false),
        271 => (0., 1., false),
        272 => (0., 4., true),
        273 => (0., 120., true),
        274..280 => (0., 1., p == 279),
        280 => (0., 2., true),
        281..283 => (0.0625, 32., false),
        283..285 => (0., 1., true),
        285..292 => (0., 127., true),
        _ => (0., 1., false),
    }
}

unsafe fn instrument<'a>(h: *mut c_void) -> &'a mut Instrument {
    &mut *(h as *mut Instrument)
}
#[no_mangle]
pub extern "C" fn lxrp_create(rate: f32) -> *mut c_void {
    Box::into_raw(Box::new(Instrument::new(rate))).cast()
}
#[no_mangle]
pub unsafe extern "C" fn lxrp_destroy(h: *mut c_void) {
    if !h.is_null() {
        drop(Box::from_raw(h as *mut Instrument));
    }
}
#[no_mangle]
pub unsafe extern "C" fn lxrp_clone(h: *mut c_void, rate: f32) -> *mut c_void {
    Box::into_raw(Box::new(instrument(h).copy_project(rate))).cast()
}
#[no_mangle]
pub unsafe extern "C" fn lxrp_set(h: *mut c_void, p: u32, v: f32) {
    instrument(h).set(p as usize, v);
}
#[no_mangle]
pub unsafe extern "C" fn lxrp_get(h: *mut c_void, p: u32) -> f32 {
    instrument(h).get(p as usize)
}
#[no_mangle]
pub unsafe extern "C" fn lxrp_command(h: *mut c_void, op: u32, args: *const f64) {
    let mut a = [0.; 8];
    a.copy_from_slice(slice::from_raw_parts(args, 8));
    if a.iter().all(|x| x.is_finite()) {
        instrument(h).command(op, a);
    }
}
#[no_mangle]
pub unsafe extern "C" fn lxrp_midi(h: *mut c_void, status: u8, key: u8, value: u8) {
    instrument(h).midi(status, key, value);
}
#[no_mangle]
pub unsafe extern "C" fn lxrp_transport(h: *mut c_void, bpm: f64, beat: f64, running: bool) {
    instrument(h).transport(bpm, beat, running);
}
#[no_mangle]
pub unsafe extern "C" fn lxrp_process(h: *mut c_void, out: *mut f32, frames: u32) {
    instrument(h).render(slice::from_raw_parts_mut(out, frames as usize * 2));
}
#[no_mangle]
pub unsafe extern "C" fn lxrp_process_multi(h: *mut c_void, out: *mut f32, frames: u32) {
    instrument(h).render_multi(slice::from_raw_parts_mut(
        out,
        frames as usize * OUTPUT_CHANNELS,
    ));
}
#[no_mangle]
pub unsafe extern "C" fn lxrp_status(h: *mut c_void, out: *mut f32) {
    let i = instrument(h);
    let a = slice::from_raw_parts_mut(out, 20);
    a[0] = i.engine.pattern as f32;
    a[1] = i.active_note.map(|x| x as f32).unwrap_or(-1.);
    a[2] = i.engine.peak[0];
    a[3] = i.engine.peak[1];
    a[4] = i.engine.bpm;
    a[5] = i.host_running as u8 as f32;
    for t in 0..7 {
        a[6 + t] = i.engine.positions[t] as f32;
        a[13 + t] = i.engine.meters[t];
    }
}
#[no_mangle]
pub unsafe extern "C" fn lxrp_preview(h: *mut c_void) -> bool {
    instrument(h).preview
}
#[no_mangle]
pub unsafe extern "C" fn lxrp_scene_enabled(h: *mut c_void, p: u32) -> bool {
    (p as usize) < PATTERNS && instrument(h).engine.scenes[p as usize].enabled
}
#[no_mangle]
pub unsafe extern "C" fn lxrp_json(h: *mut c_void, out: *mut u8, capacity: usize) -> usize {
    let text = instrument(h).project().to_string();
    if !out.is_null() && capacity >= text.len() {
        slice::from_raw_parts_mut(out, text.len()).copy_from_slice(text.as_bytes());
    }
    text.len()
}
#[no_mangle]
pub unsafe extern "C" fn lxrp_load(h: *mut c_void, data: *const u8, size: usize) -> bool {
    if size > 64_000_000 {
        return false;
    }
    serde_json::from_slice::<Value>(slice::from_raw_parts(data, size))
        .ok()
        .and_then(|v| instrument(h).load(&v))
        .is_some()
}
#[no_mangle]
pub unsafe extern "C" fn lxrp_info(p: u32, name: *mut c_char, capacity: usize, limits: *mut f32) {
    let p = p as usize;
    let (min, max, discrete) = range(p);
    let bounds = slice::from_raw_parts_mut(limits, 3);
    bounds.copy_from_slice(&[min, max, discrete as u8 as f32]);
    let label = match p {
        0..234 => format!(
            "{} {}",
            ["Drum 1", "Drum 2", "Drum 3", "Snare", "Cymbal", "Hi hats"][p / 39],
            NAMES[p % 39]
        ),
        234..249 => FX_NAMES[p - 234].into(),
        249..256 => format!("Track {} level", p - 248),
        256..263 => format!("Track {} mute", p - 255),
        263..270 => format!("Track {} solo", p - 262),
        270 => "Swing".into(),
        271 => "Morph".into(),
        272 => "Morph target".into(),
        273 => "A01 MIDI note".into(),
        274..278 => format!("Macro {}", p - 273),
        278 => "Velocity influence".into(),
        279 => "Fill".into(),
        280 => "Pattern switch quantize".into(),
        281..283 => format!("Mod LFO {} beats", p - 280),
        283 => "MIDI mode".into(),
        284 => "Output routing".into(),
        285..292 => format!(
            "{} MIDI note",
            [
                "Drum 1",
                "Drum 2",
                "Drum 3",
                "Snare",
                "Cymbal",
                "Closed hat",
                "Open hat"
            ][p - 285]
        ),
        _ => String::new(),
    };
    if capacity > 0 {
        let bytes = slice::from_raw_parts_mut(name.cast::<u8>(), capacity);
        let n = label.len().min(capacity - 1);
        bytes[..n].copy_from_slice(&label.as_bytes()[..n]);
        bytes[n] = 0;
    }
}

#[no_mangle]
pub unsafe extern "C" fn lxrp_meta(h: *mut c_void, name: *const u8, size: usize, kit: u32) {
    if size <= 240 {
        if let Ok(s) = std::str::from_utf8(slice::from_raw_parts(name, size)) {
            instrument(h).name = s.chars().take(60).collect();
            instrument(h).kit = (kit as usize).min(4);
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn lxrp_inherit(new: *mut c_void, old: *mut c_void) {
    if new != old && !old.is_null() {
        let compatible = instrument(new).midi_mode == instrument(old).midi_mode
            && instrument(new).drum_notes == instrument(old).drum_notes;
        instrument(new).inherit_transport(instrument(old));
        if compatible {
            instrument(new)
                .engine
                .take_audio(&mut instrument(old).engine);
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn lxrp_labels(h: *mut c_void, data: *const u8, size: usize) {
    if size > 4096 {
        return;
    }
    if let Ok(j) = serde_json::from_slice::<Value>(slice::from_raw_parts(data, size)) {
        let i = instrument(h);
        if j.get("kitPreset").is_some() {
            i.kit_preset = kit_preset(&j["kitPreset"]);
        }
        if let Some(names) = j["macroNames"].as_array() {
            if names.len() == 4 {
                for n in 0..4 {
                    if let Some(s) = names[n].as_str() {
                        i.macro_names[n] = s.chars().take(20).collect();
                    }
                }
            }
        }
        if let Some(depth) = j["mutation"]["depth"].as_f64() {
            if (0. ..=1.).contains(&depth) {
                if let Some(locked) = j["mutation"]["locked"].as_array() {
                    if locked.len() <= 39
                        && locked.iter().all(|n| n.as_u64().is_some_and(|n| n < 39))
                    {
                        i.mutation = j["mutation"].clone();
                    }
                }
            }
        }
    }
}
