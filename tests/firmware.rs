//! Independent reference: fixtures are produced by executing the original ARM
//! binary with tools/generate_dsp_vectors.py in the adjacent reverse project.
use lxr02::firmware::{
    envelope::{Decay, Envelope},
    filter::Filter,
    oscillator::Oscillator,
    transient::Transient,
    RATE,
};
struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        assert_eq!(&data[..4], b"LXRF");
        Self { data, at: 4 }
    }
    fn bytes(&mut self, n: usize) -> &'a [u8] {
        let p = self.at;
        self.at += n;
        &self.data[p..p + n]
    }
    fn u8(&mut self) -> u8 {
        self.bytes(1)[0]
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.bytes(4).try_into().unwrap())
    }
    fn i16(&mut self) -> i16 {
        i16::from_le_bytes(self.bytes(2).try_into().unwrap())
    }
    fn f32(&mut self) -> f32 {
        f32::from_bits(self.u32())
    }
    fn f64(&mut self) -> f64 {
        f64::from_le_bytes(self.bytes(8).try_into().unwrap())
    }
    fn pcm(&mut self, n: usize) -> Vec<i16> {
        (0..n).map(|_| self.i16()).collect()
    }
    fn floats<const N: usize>(&mut self) -> [f32; N] {
        std::array::from_fn(|_| self.f32())
    }
    fn done(self) {
        assert_eq!(self.at, self.data.len());
    }
}
fn equal_pcm(actual: &[i16], expected: &[i16], context: &str) {
    let mismatch = actual
        .iter()
        .zip(expected)
        .enumerate()
        .find(|(_, (a, b))| a != b);
    if let Some((i, (a, b))) = mismatch {
        panic!(
            "{context}, sample {i}: Rust {a}, ARM {b}; max error {}",
            actual
                .iter()
                .zip(expected)
                .map(|(&a, &b)| (a as i32 - b as i32).abs())
                .max()
                .unwrap()
        );
    }
}
#[test]
fn original_oscillator_and_unipolar_fm_pcm_and_phase() {
    let mut r = Reader::new(include_bytes!("fixtures/oscillator.bin"));
    for case in 0..r.u32() {
        let wave = r.u8();
        let bank = r.u8();
        let phase = r.u32();
        let increment = r.u32();
        let n = r.u32() as usize;
        let amp = r.f32();
        let width = r.f32();
        let fm = r.f32();
        let modulated = r.i16() != 0;
        let modulation = r.pcm(n);
        let expected = r.pcm(n);
        let final_phase = r.u32();
        let held = r.i16();
        let mut o = Oscillator {
            phase,
            increment,
            wave,
            bank,
            held: 123,
            fm,
        };
        let mut rng = 0x1982u32;
        let actual: Vec<i16> = modulation
            .iter()
            .map(|&m| {
                o.tick(amp, width, modulated.then_some(m), || {
                    rng ^= rng << 13;
                    rng ^= rng >> 17;
                    rng ^= rng << 5;
                    rng as i16
                })
            })
            .collect();
        equal_pcm(
            &actual,
            &expected,
            &format!("oscillator {case}, wave {wave}, bank {bank}, FM {modulated}"),
        );
        assert_eq!(o.phase, final_phase, "phase {case}");
        assert_eq!(o.held, held, "held {case}");
    }
    r.done();
}
#[test]
fn original_frequency_setter_and_bank_boundaries() {
    let mut r = Reader::new(include_bytes!("fixtures/tuning.bin"));
    for _ in 0..r.u32() {
        let wave = r.u8();
        let hz = r.f32();
        let increment = r.u32();
        let bank = r.u8();
        let mut o = Oscillator {
            wave,
            ..Default::default()
        };
        o.tune(hz, RATE);
        assert_eq!(o.increment, increment, "increment wave {wave} {hz}Hz");
        assert_eq!(o.bank, bank, "bank wave {wave} {hz}Hz");
    }
    r.done();
}
#[test]
fn original_nonlinear_filter_all_modes_pcm_and_state() {
    let mut r = Reader::new(include_bytes!("fixtures/filter.bin"));
    for case in 0..r.u32() {
        let kind = r.u8();
        let state = r.floats();
        let n = r.u32() as usize;
        let input = r.pcm(n);
        let expected = r.pcm(n);
        let final_state = r.floats::<8>();
        let mut filter = Filter { state };
        let actual: Vec<i16> = input.into_iter().map(|x| filter.tick(x, kind)).collect();
        equal_pcm(&actual, &expected, &format!("filter {case} mode {kind}"));
        for i in 0..8 {
            assert_eq!(
                filter.state[i].to_bits(),
                final_state[i].to_bits(),
                "state {case} mode {kind} field {i}: {} vs {}",
                filter.state[i],
                final_state[i]
            );
        }
    }
    r.done();
}
#[test]
fn all_original_transients_pcm_rate_and_end_of_sample() {
    let mut r = Reader::new(include_bytes!("fixtures/transient.bin"));
    for case in 0..r.u32() {
        let kind = r.u8();
        let position = r.u32();
        let rate = r.f32();
        let volume = r.f32();
        let n = r.u32() as usize;
        let expected = r.pcm(n);
        let end = r.u32();
        let mut t = Transient {
            kind,
            position,
            rate,
            volume,
            ..Default::default()
        };
        let actual: Vec<i16> = (0..n).map(|_| t.tick()).collect();
        equal_pcm(&actual, &expected, &format!("transient {case} kind {kind}"));
        assert_eq!(t.position, end);
    }
    r.done();
}
#[test]
fn original_snappy_pitch_transient() {
    let mut r = Reader::new(include_bytes!("fixtures/snappy.bin"));
    for _ in 0..r.u32() {
        let rate = r.f32();
        let n = r.u32();
        let mut t = Transient {
            snappy: 1.,
            ..Default::default()
        };
        for i in 0..n {
            assert_eq!(t.pitch(rate).to_bits(), r.u32(), "snappy {rate} at {i}");
        }
    }
    r.done();
}
#[test]
fn original_control_envelopes_including_repeat_and_overshoot() {
    let mut r = Reader::new(include_bytes!("fixtures/envelope.bin"));
    for case in 0..r.u32() {
        let addr = r.u32();
        let n = r.u32();
        let mut init = Reader {
            data: r.bytes(56),
            at: 0,
        };
        if addr == 0x4bc {
            let mut e = Decay {
                delta: init.f32(),
                slope: init.f32(),
                level: init.f32(),
            };
            for i in 0..n {
                assert_eq!(e.tick().to_bits(), r.u32(), "decay {case} at {i}");
            }
        } else {
            let mut e = Envelope {
                attack: init.f64(),
                decay: init.f64(),
                decay_slope: init.f64(),
                attack_slope: init.f64(),
                ..Default::default()
            };
            e.repeats = init.u8();
            e.remaining = init.u8();
            init.bytes(6);
            e.level = init.f64();
            e.stage = init.u8();
            for i in 0..n {
                assert_eq!(e.tick().to_bits(), r.u32(), "amp envelope {case} at {i}");
            }
        }
        r.bytes(56);
    }
    r.done();
}
#[test]
fn original_tube_fold_clip_pcm_and_integrator_states() {
    use lxr02::firmware::effects::Drive;
    let mut r = Reader::new(include_bytes!("fixtures/drive.bin"));
    for case in 0..r.u32() {
        let mut d = Drive {
            kind: r.u8(),
            amount: r.f32(),
            shape: r.f32(),
            tone: r.f32(),
            level: r.f32(),
            ..Default::default()
        };
        let mix = r.f32();
        let n = r.u32() as usize;
        let input = r.pcm(n * 2);
        let expected = r.pcm(n * 2);
        let states = r.bytes(128);
        let actual: Vec<i16> = input
            .chunks_exact(2)
            .flat_map(|x| d.tick([x[0], x[1]], mix))
            .collect();
        equal_pcm(&actual, &expected, &format!("drive {case} mode {}", d.kind));
        for (base, values) in [
            (0x18, d.fast_lp),
            (0x38, d.slow_lp),
            (0x48, d.fast_hp),
            (0x58, d.slow_hp),
            (0x68, d.smooth),
            (0x78, d.slew),
            (0x88, d.feedback),
        ] {
            for (c, value) in values.iter().enumerate() {
                let offset = base - 0x18 + c * 8;
                let arm = f64::from_le_bytes(states[offset..offset + 8].try_into().unwrap());
                assert_eq!(
                    value.to_bits(),
                    arm.to_bits(),
                    "drive {case} state {base:x} channel {c}: {value} vs {arm}"
                );
            }
        }
    }
    r.done();
}
#[test]
fn original_compressor_pcm_and_db_envelope() {
    use lxr02::firmware::effects::Compressor;
    let mut r = Reader::new(include_bytes!("fixtures/compressor.bin"));
    for case in 0..r.u32() {
        let mut c = Compressor {
            threshold: r.f32(),
            attack: r.f32(),
            release: r.f32(),
            ratio: r.f32(),
            makeup: r.f32(),
            envelope: 0.,
        };
        let mix = r.f32();
        let n = r.u32() as usize;
        let input = r.pcm(n * 2);
        let expected = r.pcm(n * 2);
        let state = r.f64();
        let actual: Vec<i16> = input
            .chunks_exact(2)
            .flat_map(|x| c.tick([x[0], x[1]], mix))
            .collect();
        equal_pcm(&actual, &expected, &format!("compressor {case}"));
        assert_eq!(
            c.envelope.to_bits(),
            state.to_bits(),
            "compressor state {case}: {} vs {state}",
            c.envelope
        );
    }
    r.done();
}
#[test]
fn original_ring_modulation_pcm_and_phase() {
    use lxr02::firmware::effects::Ring;
    let mut r = Reader::new(include_bytes!("fixtures/ring.bin"));
    for case in 0..r.u32() {
        let wave = r.u8();
        let hz = r.f32();
        let mix = r.f32();
        let n = r.u32() as usize;
        let input = r.pcm(n * 2);
        let expected = r.pcm(n * 2);
        let phase = r.u32();
        let mut ring = Ring::default();
        ring.oscillator.wave = wave;
        ring.configure(hz, RATE);
        let actual: Vec<i16> = input
            .chunks_exact(2)
            .flat_map(|x| ring.tick([x[0], x[1]], mix))
            .collect();
        equal_pcm(&actual, &expected, &format!("ring {case}"));
        assert_eq!(ring.oscillator.phase, phase);
    }
    r.done();
}
#[test]
fn original_per_voice_rational_shaper() {
    use lxr02::firmware::{effects::Shaper, OnePole};
    let mut r = Reader::new(include_bytes!("fixtures/shaper.bin"));
    for case in 0..r.u32() {
        let amount = r.f32();
        let coefficient = r.f64();
        let n = r.u32() as usize;
        let input = r.pcm(n);
        let expected = r.pcm(n);
        let state = r.f64();
        let mut d = Shaper {
            amount,
            smooth: OnePole {
                coefficient,
                state: 0.,
            },
        };
        let actual: Vec<i16> = input.into_iter().map(|x| d.tick(x)).collect();
        equal_pcm(&actual, &expected, &format!("shaper {case}"));
        assert_eq!(d.smooth.state.to_bits(), state.to_bits());
    }
    r.done();
}
#[test]
fn firmware_math_matches_original_logf_expf() {
    use lxr02::firmware::math;
    let mut r = Reader::new(include_bytes!("fixtures/math.bin"));
    for case in 0..r.u32() {
        let addr = r.u32();
        let x = r.f32();
        let expected = r.u32();
        let y = if addr == 0x08029888 {
            math::exp(x)
        } else {
            math::log(x)
        };
        assert_eq!(
            y.to_bits(),
            expected,
            "math {addr:x} case {case} input {x}: {} vs {}",
            y,
            f32::from_bits(expected)
        );
    }
    r.done();
}
#[test]
fn original_half_precision_delays_pcm_buffer_and_time_crossfades() {
    use lxr02::firmware::delay::Delay;
    let mut r = Reader::new(include_bytes!("fixtures/delay.bin"));
    for case in 0..r.u32() {
        let ping_pong = r.u8() != 0;
        let range = r.f32();
        let time = r.f32();
        let tone = r.f32();
        let feedback = r.f32();
        let mix = r.f32();
        let n = r.u32() as usize;
        let input = r.pcm(n * 2);
        let expected = r.pcm(n * 2);
        let state = r.bytes(96);
        let line = r.bytes(65536);
        let mut d = Delay {
            ping_pong,
            feedback,
            ..Default::default()
        };
        d.set_tone(tone);
        d.set_range(range);
        d.set_time(time);
        let mut actual = Vec::new();
        for (i, x) in input.chunks_exact(2).enumerate() {
            if i == 64 {
                d.set_time(time * 0.71);
            }
            if i == 1536 {
                d.set_time(time * 0.39);
            }
            actual.extend(d.tick([x[0], x[1]], mix));
        }
        equal_pcm(
            &actual,
            &expected,
            &format!("delay {case}, ping {ping_pong}"),
        );
        let mut s = Reader { data: state, at: 0 };
        assert_eq!(d.read, s.u32());
        assert_eq!(d.next, s.u32());
        assert_eq!(d.write, s.u32());
        for x in [d.fraction, d.old_fraction, d.fade, d.fade_step] {
            assert_eq!(x.to_bits(), s.u32(), "delay {case} state at {}", s.at - 4);
        }
        for x in d.last {
            assert_eq!(x, s.i16(), "delay {case} last");
        }
        for x in [d.feedback, d.range, d.time, d.pending_time] {
            assert_eq!(x.to_bits(), s.u32());
        }
        assert_eq!(d.pending, s.u8() != 0);
        s.bytes(3);
        for t in d.tone {
            assert_eq!(t.coefficient.to_bits(), s.f64().to_bits());
            assert_eq!(t.state.to_bits(), s.f64().to_bits(), "delay tone {case}");
        }
        let expected_line: Vec<u16> = line
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes(b.try_into().unwrap()))
            .collect();
        assert_eq!(d.line, expected_line, "delay {case} half buffer");
    }
    r.done();
}
#[test]
fn complete_drum_voice_matches_original_control_and_audio_pipeline() {
    use lxr02::dsp::{Sine, Voice};
    let sine = Sine::default();
    let mut r = Reader::new(include_bytes!("fixtures/drum-voice.bin"));
    for case in 0..r.u32() {
        let p = r.floats::<39>();
        let velocity = r.f32();
        let n = r.u32() as usize;
        let expected = r.pcm(n);
        let mut voice = Voice::new(0);
        voice.p = p;
        voice.trigger(0, velocity, 0, RATE);
        let actual: Vec<i16> = (0..n)
            .map(|_| (voice.tick(0, RATE, &sine) * 32768.) as i16)
            .collect();
        equal_pcm(
            &actual,
            &expected,
            &format!(
                "complete drum {case}, wave {} mode {} transient {}",
                p[1], p[32], p[27]
            ),
        );
    }
    r.done();
}
#[test]
fn complete_snare_cymbal_and_hat_voices_match_original_pipeline() {
    use lxr02::dsp::{Sine, Voice};
    let sine = Sine::default();
    let mut r = Reader::new(include_bytes!("fixtures/percussion-voice.bin"));
    for case in 0..r.u32() {
        let index = r.u8() as usize;
        let p = r.floats::<39>();
        let velocity = r.f32();
        let n = r.u32() as usize;
        let expected = r.pcm(n);
        let mut voice = Voice::new(index);
        voice.p = p;
        voice.trigger(index, velocity, 0, RATE);
        let actual: Vec<i16> = (0..n)
            .map(|_| (voice.tick(index, RATE, &sine) * 32768.) as i16)
            .collect();
        equal_pcm(
            &actual,
            &expected,
            &format!(
                "complete percussion {case}, index {index} wave {} transient {}",
                p[1], p[27]
            ),
        );
    }
    r.done();
}
