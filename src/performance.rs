//! Performance extensions share the same scheduler in WASM and native plugins.
use crate::params::{PARAMS, TRACKS, VOICES};
use std::sync::Arc;

pub const DIVISIONS: [f64; 13] = [
    4.,
    2.,
    1.,
    0.5,
    0.25,
    0.125,
    0.0625,
    2. / 3.,
    1. / 3.,
    1. / 6.,
    1. / 12.,
    0.75,
    0.375,
];
pub const ROUTES: usize = 16;
#[derive(Clone, Copy)]
pub struct Route {
    pub source: u8,
    pub voice: u8,
    pub param: u8,
    pub amount: f32,
    pub source_voice: u8,
    pub source_param: u8,
    pub configured: bool,
}
impl Default for Route {
    fn default() -> Self {
        Self {
            source: 0,
            voice: 0,
            param: 12,
            amount: 0.,
            source_voice: 0,
            source_param: 12,
            configured: false,
        }
    }
}
#[derive(Clone, Copy)]
pub struct Scene {
    pub enabled: bool,
    pub params: [[f32; PARAMS]; VOICES],
    pub fx: [f32; 15],
    pub levels: [f32; TRACKS],
    pub muted: [bool; TRACKS],
    pub solo: [bool; TRACKS],
    pub macros: [f32; 4],
    pub morph: f32,
    pub morph_target: usize,
}
impl Default for Scene {
    fn default() -> Self {
        Self {
            enabled: false,
            params: crate::params::kit(0),
            fx: [
                0., 0.22, 0.25, 0.35, 0.55, 0.25, 0.8, 0., 1., 0., 0.25, 0.5, 0.75, 4., 1.,
            ],
            levels: [0.85; TRACKS],
            muted: [false; TRACKS],
            solo: [false; TRACKS],
            macros: [0.; 4],
            morph: 0.,
            morph_target: 1,
        }
    }
}
#[derive(Clone)]
pub struct Sample {
    pub pcm: Arc<[f32]>,
    pub rate: f32,
    pub gain: f32,
    pub tune: f32,
    pub start: f32,
    pub end: f32,
    pub decay: f32,
    pub replace: bool,
    position: f64,
    increment: f64,
    envelope: f32,
    factor: f32,
    active: bool,
}
impl Default for Sample {
    fn default() -> Self {
        Self {
            pcm: Arc::from([]),
            rate: 44100.,
            gain: 0.7,
            tune: 0.,
            start: 0.,
            end: 1.,
            decay: 4.,
            replace: false,
            position: 0.,
            increment: 1.,
            envelope: 0.,
            factor: 1.,
            active: false,
        }
    }
}
impl Sample {
    pub fn trigger(&mut self, velocity: f32, note: i32) {
        self.position = self.start as f64 * self.pcm.len() as f64;
        self.increment = self.rate as f64 / crate::firmware::RATE as f64
            * 2f64.powf((self.tune as f64 + note as f64) / 12.);
        self.envelope = velocity.clamp(0., 1.);
        self.factor = (-1. / (self.decay.max(0.005) * crate::firmware::RATE)).exp();
        self.active = !self.pcm.is_empty();
    }
    pub fn stop(&mut self) {
        self.active = false;
    }
    pub fn inherit_playback(&mut self, old: &Self) {
        if Arc::ptr_eq(&self.pcm, &old.pcm) {
            self.position = old.position;
            self.increment = old.increment;
            self.envelope = old.envelope;
            self.factor = old.factor;
            self.active = old.active;
        }
    }
    pub fn tick(&mut self) -> f32 {
        if !self.active {
            return 0.;
        }
        let end = (self.end as f64 * self.pcm.len() as f64).min(self.pcm.len() as f64);
        if self.position >= end || self.envelope < 0.00001 {
            self.active = false;
            return 0.;
        }
        let i = self.position as usize;
        let frac = (self.position - i as f64) as f32;
        let a = self.pcm.get(i).copied().unwrap_or(0.);
        let b = self.pcm.get(i + 1).copied().unwrap_or(0.);
        let x = (a + (b - a) * frac) * self.envelope * self.gain;
        // A short fade at both trim edges avoids clicks in arbitrary user samples.
        let edge = ((end - self.position) / 64.)
            .min((self.position - self.start as f64 * self.pcm.len() as f64 + 1.) / 32.)
            .clamp(0., 1.) as f32;
        self.position += self.increment;
        self.envelope *= self.factor;
        x * edge
    }
}
#[derive(Clone, Copy, Default, Debug)]
pub struct Hit {
    pub frame: u64,
    pub track: u8,
    pub note: i8,
    pub velocity: f32,
}
#[derive(Clone, Copy)]
pub struct TrackClock {
    pub index: i64,
    pub due: f64,
    pub previous: bool,
    pub cycle_origin: i64,
}
impl Default for TrackClock {
    fn default() -> Self {
        Self {
            index: 0,
            due: 0.,
            previous: false,
            cycle_origin: 0,
        }
    }
}

/// Event times stay ordered: the combined micro/groove offset is below half a step.
pub fn event_beat(track: &crate::sequencer::Track, index: i64, swing: f32) -> f64 {
    let unit = DIVISIONS[track.division.min(12) as usize];
    let step = track.steps[index.rem_euclid(track.length as i64) as usize];
    let groove = match track.groove {
        1 => {
            if index.rem_euclid(2) == 1 {
                0.24
            } else {
                0.
            }
        }
        2 => [0., 0.12, -0.08, 0.28][index.rem_euclid(4) as usize],
        3 => [0., -0.18, 0.04, -0.12][index.rem_euclid(4) as usize],
        4 => [0., 0.2, 0.08, 0.3][index.rem_euclid(4) as usize],
        _ => 0.,
    };
    let offset = (step.micro
        + track.delay
        + groove * track.groove_amount
        + if index.rem_euclid(2) == 1 {
            (swing + track.swing).clamp(0., 0.5)
        } else {
            0.
        })
    .clamp(-0.49, 0.5);
    index as f64 * unit + offset as f64 * unit
}
pub fn condition(kind: u8, every: u8, phase: u8, cycle: i64, fill: bool, previous: bool) -> bool {
    match kind {
        1 => cycle == 0,
        2 => cycle != 0,
        3 => fill,
        4 => !fill,
        5 => previous,
        6 => !previous,
        7 => cycle.rem_euclid(every.max(1) as i64) == (phase % every.max(1)) as i64,
        _ => true,
    }
}
