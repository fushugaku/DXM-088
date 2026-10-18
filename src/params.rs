//! Parameter units belong to the web instrument, not the firmware's MIDI CC scale.
pub const VOICES: usize = 6;
pub const TRACKS: usize = 7;
pub const PARAMS: usize = 39;
pub const PITCH: usize = 0;
pub const WAVE: usize = 1;
pub const DECAY: usize = 2;
pub const ATTACK: usize = 3;
pub const SLOPE: usize = 4;
pub const PITCH_AMOUNT: usize = 5;
pub const PITCH_DECAY: usize = 6;
pub const FM_RATIO: usize = 7;
pub const FM_AMOUNT: usize = 8;
pub const FM_WAVE: usize = 9;
pub const NOISE: usize = 10;
pub const CLICK: usize = 11;
pub const CUTOFF: usize = 12;
pub const RESONANCE: usize = 13;
pub const FILTER: usize = 14;
pub const DRIVE: usize = 15;
pub const DISTORTION: usize = 16;
pub const REDUCTION: usize = 17;
pub const PAN: usize = 18;
pub const LEVEL: usize = 19;
pub const LFO_RATE: usize = 20;
pub const LFO_DEPTH: usize = 21;
pub const LFO_TARGET: usize = 22;
pub const LFO_WAVE: usize = 23;
pub const FM_RATIO2: usize = 24;
pub const FM_AMOUNT2: usize = 25;
pub const PWM: usize = 26;
pub const TRANSIENT: usize = 27;
pub const REPEAT: usize = 28;
pub const OPEN_DECAY: usize = 29;
pub const VELOCITY_TARGET: usize = 30;
pub const VELOCITY_AMOUNT: usize = 31;
pub const FM_MODE: usize = 32;
pub const LFO_RETRIGGER: usize = 33;
pub const CLICK_FREQ: usize = 34;
pub const NOISE_FREQ: usize = 35;
pub const FILTER_DRIVE: usize = 36;
pub const PITCH_SLOPE: usize = 37;
pub const FM_WAVE2: usize = 38;

pub const MIN: [f32; PARAMS] = [
    20., 0., 0.005, 0., 0.005, -36., 0.005, 0.125, 0., 0., 0., 0., 20., 0., 0., 0., 0., 0., -1.,
    0., 0.01, 0., 0., 0., 0.125, 0., 0.05, 0., 0., 0.01, 0., -1., 0., 0., 0.25, 100., 0., 0.005,
    0.,
];
pub const MAX: [f32; PARAMS] = [
    8000., 5., 4., 1., 0.995, 72., 2., 16., 1., 5., 1., 1., 20000., 1., 7., 1., 3., 1., 1., 1.25,
    40., 1., 3., 4., 16., 1., 0.95, 13., 8., 4., 4., 1., 1., 1., 4., 20000., 1., 0.995, 5.,
];
pub const DISCRETE: [usize; 12] = [
    WAVE,
    FM_WAVE,
    FILTER,
    DISTORTION,
    LFO_TARGET,
    LFO_WAVE,
    TRANSIENT,
    REPEAT,
    VELOCITY_TARGET,
    FM_MODE,
    LFO_RETRIGGER,
    FM_WAVE2,
];

pub fn constrain(id: usize, value: f32) -> Option<f32> {
    if id >= PARAMS || !value.is_finite() {
        return None;
    }
    let v = value.clamp(MIN[id], MAX[id]);
    Some(if DISCRETE.contains(&id) { v.round() } else { v })
}

pub fn kit(index: usize) -> [[f32; PARAMS]; VOICES] {
    let mut p = [[0.0_f32; PARAMS]; VOICES];
    for (v, a) in p.iter_mut().enumerate() {
        *a = [
            90., 0., 0.3, 0., 0.27, 0., 0.06, 2., 0., 0., 0., 0.12, 16000., 0.1, 1., 0.12, 1., 0.,
            0., 0.8, 1., 0., 1., 0., 3.41, 0.6, 0.5, 0., 0., 0.55, 0., 0., 0., 1., 1., 10000., 0.,
            0.22, 4.,
        ];
        a[PITCH] = [49., 135., 240., 178., 320., 690.][v];
        a[DECAY] = [0.48, 0.24, 0.18, 0.19, 0.16, 0.055][v];
        a[LEVEL] = [1., 0.62, 0.55, 0.68, 0.57, 0.44][v];
        a[PAN] = [0., -0.18, 0.2, 0., -0.12, 0.24][v];
        if v < 3 {
            a[PITCH_AMOUNT] = [40., 22., 12.][v];
            a[FM_AMOUNT] = [0.08, 0.45, 1.8][v];
            a[FM_RATIO] = [1., 2.76, 3.5][v];
        } else if v == 3 {
            a[NOISE] = 0.72;
            a[CUTOFF] = 5200.;
            a[FILTER] = 3.;
            a[RESONANCE] = 0.18;
            a[PITCH_AMOUNT] = 7.;
            a[CLICK] = 0.24;
        } else if v == 4 {
            a[WAVE] = 1.;
            a[NOISE] = 0.92;
            a[CUTOFF] = 1700.;
            a[FILTER] = 3.;
            a[RESONANCE] = 0.25;
            a[REPEAT] = 3.;
            a[ATTACK] = 0.035;
            a[CLICK] = 0.;
            a[FM_AMOUNT] = 0.75;
            a[FM_RATIO] = 2.71;
            a[FM_AMOUNT2] = 0.9;
        } else {
            a[WAVE] = 3.;
            a[NOISE] = 0.4;
            a[CUTOFF] = 6800.;
            a[FILTER] = 2.;
            a[FM_RATIO] = 1.37;
            a[FM_RATIO2] = 2.83;
            a[FM_AMOUNT] = 1.;
            a[CLICK] = 0.03;
            a[SLOPE] = 0.18;
        }
    }
    match index % 5 {
        1 => {
            p[0][PITCH] = 44.;
            p[0][DECAY] = 0.67;
            p[0][DRIVE] = 0.34;
            p[1][PITCH] = 196.;
            p[1][FM_AMOUNT] = 4.2;
            p[1][FM_RATIO] = 1.414;
            p[2][PITCH] = 280.;
            p[2][FM_AMOUNT] = 5.1;
            p[2][FM_RATIO] = 2.71;
            p[4][WAVE] = 3.;
            p[4][NOISE] = 0.12;
            p[4][REPEAT] = 0.;
            p[4][DECAY] = 0.64;
            p[4][CUTOFF] = 2400.;
            p[4][FILTER] = 2.;
            p[4][FM_RATIO2] = 4.13;
            p[5][OPEN_DECAY] = 0.8;
            p[5][CUTOFF] = 8100.;
        }
        2 => {
            p[0][PITCH] = 55.;
            p[0][DECAY] = 0.36;
            p[0][CLICK] = 0.4;
            p[1][WAVE] = 1.;
            p[1][PITCH] = 93.;
            p[1][FM_AMOUNT] = 0.1;
            p[2][PITCH] = 440.;
            p[2][FM_RATIO] = 1.5;
            p[2][FM_AMOUNT] = 3.5;
            p[3][NOISE] = 0.57;
            p[3][DECAY] = 0.15;
            p[4][CUTOFF] = 2100.;
            p[4][DECAY] = 0.1;
            p[4][REPEAT] = 4.;
            p[5][DECAY] = 0.035;
            p[5][NOISE] = 0.72;
            p[5][OPEN_DECAY] = 0.34;
        }
        3 => {
            for a in &mut p {
                a[REDUCTION] = 0.46;
                a[DISTORTION] = 2.;
                a[DRIVE] = 0.26;
            }
            p[0][PITCH] = 42.;
            p[0][DECAY] = 0.65;
            p[0][FM_AMOUNT] = 0.52;
            p[1][WAVE] = 2.;
            p[1][FM_RATIO] = 0.5;
            p[1][FM_AMOUNT] = 2.4;
            p[2][FM_AMOUNT] = 6.8;
            p[2][FM_RATIO] = 1.618;
            p[3][CUTOFF] = 2800.;
            p[4][CUTOFF] = 1200.;
            p[5][CUTOFF] = 4700.;
            p[5][OPEN_DECAY] = 0.43;
        }
        4 => {
            for a in &mut p {
                a[FM_AMOUNT] = 0.;
                a[CLICK] = 0.;
                a[NOISE] = 0.;
                a[FILTER] = 0.;
                a[DRIVE] = 0.;
                a[DISTORTION] = 0.;
                a[REDUCTION] = 0.;
                a[REPEAT] = 0.;
            }
        }
        _ => {}
    }
    // Original modulation amplitudes are normalized, unlike the first web build.
    for (index, a) in p.iter_mut().enumerate() {
        a[FM_AMOUNT] = if index < 3 {
            a[FM_AMOUNT] / 16.
        } else {
            a[FM_AMOUNT].min(1.)
        };
        a[FM_AMOUNT2] /= 2.;
    }
    p
}
