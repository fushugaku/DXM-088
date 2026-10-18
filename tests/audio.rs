use lxr02::{dsp::envelope_curve, params::*, sequencer::*, Engine};

fn energy(e: &mut Engine, frames: usize) -> f64 {
    let mut sum = 0.;
    let mut left = frames;
    while left > 0 {
        let n = left.min(128);
        for &x in e.render(n) {
            assert!(x.is_finite());
            assert!(x.abs() <= 1.001);
            sum += (x as f64).powi(2);
        }
        left -= n;
    }
    sum
}
#[test]
fn silence_before_trigger() {
    let mut e = Engine::new(44100.);
    assert_eq!(energy(&mut e, 4096), 0.);
}
#[test]
fn every_voice_produces_sound_and_decays() {
    for t in 0..7 {
        let mut e = Engine::new(44100.);
        e.trigger(t, 1., 0);
        let signal_energy = energy(&mut e, 44100);
        assert!(signal_energy > 0.1, "track {t}: energy {signal_energy}");
        energy(&mut e, 44100 * 4);
        assert!(energy(&mut e, 4096) < 0.00001, "tail {t}");
    }
}
#[test]
fn sequencer_is_sample_accurate_at_two_rates() {
    for sr in [44100., 48000.] {
        let mut e = Engine::new(sr);
        e.tempo(120., 0.);
        e.play();
        energy(&mut e, sr as usize * 2);
        assert_eq!(e.tick_count, 16);
        energy(&mut e, 1);
        assert_eq!(e.tick_count, 17);
    }
}
#[test]
fn swing_preserves_pair_duration() {
    let mut e = Engine::new(48000.);
    e.tempo(120., 0.4);
    e.play();
    energy(&mut e, 48000);
    assert_eq!(e.tick_count, 8);
}
#[test]
fn queued_pattern_changes_on_boundary() {
    let mut e = Engine::new(48000.);
    e.tempo(120., 0.);
    e.play();
    e.select_pattern(3);
    energy(&mut e, 96000);
    assert_eq!(e.pattern, 0);
    energy(&mut e, 1);
    assert_eq!(e.pattern, 3);
}
#[test]
fn euclidean_has_exact_pulse_count() {
    for n in 1..=64 {
        for k in 0..=n {
            let mut track = Track::default();
            euclidean(&mut track, k, n, 3);
            assert_eq!(track.steps.iter().filter(|s| s.velocity > 0.).count(), k);
        }
    }
}
#[test]
fn probability_zero_is_silent() {
    let mut e = Engine::new(44100.);
    for t in 0..TRACKS {
        for s in 0..64 {
            e.set_step(0, t, s, 1., 0., 0, 1);
        }
    }
    e.play();
    assert_eq!(energy(&mut e, 44100), 0.);
}
#[test]
fn mute_and_solo_gate_the_mix() {
    let mut e = Engine::new(44100.);
    e.muted = [true; TRACKS];
    e.play();
    assert_eq!(energy(&mut e, 22050), 0.);
    e.muted[0] = false;
    assert!(energy(&mut e, 44100) > 0.1);
    e.solo[6] = true;
    energy(&mut e, 22050);
    assert!(energy(&mut e, 22050) < 1e-7);
}
#[test]
fn all_kits_and_effects_are_finite_at_supported_rates() {
    for sr in [8000., 44100., 48000., 96000.] {
        for k in 0..5 {
            let mut e = Engine::new(sr);
            e.load_kit(k);
            e.play();
            for fx in 0..8 {
                e.fx.kind = fx;
                e.fx.mix = 0.65;
                assert!(energy(&mut e, 4096) > 0.);
            }
        }
    }
}
#[test]
fn unsafe_values_are_rejected_or_bounded() {
    let mut e = Engine::new(f32::NAN);
    e.set_param(0, PITCH, f32::NAN);
    e.set_param(0, 999, 1.);
    e.set_param(0, CUTOFF, f32::INFINITY);
    e.tempo(f32::NAN, 0.);
    e.set_fx(3, f32::INFINITY);
    e.trigger(999, 1., 0);
    e.generate(999, 999, 999, 0, 999);
    e.set_step(0, 0, 0, 1., 1., i32::MAX, u32::MAX);
    e.play();
    assert!(energy(&mut e, 4096) > 0.);
}
#[test]
fn pitch_lock_changes_audio_without_modifying_the_kit() {
    let mut a = Engine::new(44100.);
    let mut b = Engine::new(44100.);
    b.set_lock(0, 0, 0, 0, PITCH, 300.);
    a.play();
    b.play();
    let first = a.render(1024).to_vec();
    let second = b.render(1024).to_vec();
    assert_ne!(first, second);
    assert_eq!(b.params[0][PITCH], 49.);
}
#[test]
fn ratchets_change_the_render() {
    let mut a = Engine::new(44100.);
    let mut b = Engine::new(44100.);
    b.set_step(0, 0, 0, 0.95, 1., 0, 4);
    a.play();
    b.play();
    let mut difference = 0.;
    for _ in 0..50 {
        let first = a.render(128).to_vec();
        let second = b.render(128);
        difference += first
            .iter()
            .zip(second)
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>();
    }
    assert!(difference > 1.);
}
#[test]
fn full_morph_matches_the_target_parameters() {
    let mut a = Engine::new(44100.);
    let mut b = Engine::new(44100.);
    a.set_morph(1., 1);
    b.load_kit(1);
    a.play();
    b.play();
    assert_eq!(a.render(4096), b.render(4096));
}
#[test]
fn hat_uses_one_choking_voice() {
    let mut e = Engine::new(44100.);
    e.trigger(6, 1., 0);
    energy(&mut e, 8000);
    e.trigger(5, 1., 0);
    energy(&mut e, 22050);
    assert!(energy(&mut e, 4096) < 1e-8);
}
#[test]
fn recovered_curve_is_bounded_and_monotonic() {
    for s in [0.005, 0.2, 0.5, 0.8, 0.995] {
        let mut prev = 0.;
        for i in 0..=1000 {
            let y = envelope_curve(i as f32 / 1000., s);
            assert!(y >= prev - 1e-6 && y <= 1.00001);
            prev = y;
        }
        assert!((prev - 1.).abs() < 1e-5);
    }
}
#[test]
fn render_is_independent_of_block_size() {
    let mut a = Engine::new(44100.);
    let mut b = Engine::new(44100.);
    a.play();
    b.play();
    let expected = a.render(4096).to_vec();
    let mut actual = vec![];
    for _ in 0..32 {
        actual.extend_from_slice(b.render(128));
    }
    assert_eq!(expected, actual);
}
