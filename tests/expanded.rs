use lxr02::{
    dsp::{Sine, Voice},
    params::*,
    rustias_filter::Model,
    Engine,
};

fn audio(mut e: Engine, trigger_source: bool) -> Vec<f32> {
    // Isolate the destination; the source envelope still runs while muted.
    e.set_mix(0, 0, 0.);
    e.render(4096);
    if trigger_source {
        e.trigger(0, 1., 0);
    }
    e.trigger(1, 1., 0);
    let mut result = Vec::new();
    for _ in 0..6 {
        result.extend_from_slice(e.render(1024));
    }
    assert!(result.iter().all(|x| x.is_finite()));
    result
}

#[test]
fn three_envelopes_have_independent_timing_and_retrigger() {
    let sine = Sine::default();
    let mut voice = Voice::new(0);
    voice.p[DECAY] = 0.5;
    voice.p[FILTER_ENV_ATTACK] = 0.01;
    voice.p[FILTER_ENV_DECAY] = 0.025;
    voice.p[ASSIGN_ENV_ATTACK] = 0.;
    voice.p[ASSIGN_ENV_DECAY] = 0.2;
    voice.trigger(0, 1., 0, 48000.);
    for _ in 0..4800 {
        voice.tick(0, 48000., &sine);
    }
    let [amp, filter, assign] = voice.envelope_levels();
    assert!(amp > 0.);
    assert_eq!(filter, 0.);
    assert!(assign > 0.);
    voice.trigger(0, 1., 0, 48000.);
    voice.tick(0, 48000., &sine);
    assert!(voice.envelope_levels()[1] > 0.);
    assert!(voice.envelope_levels()[2] > assign);
}

#[test]
fn rustias_models_remain_bounded_at_extreme_settings() {
    for mode in 1..=17 {
        for rate in [44140.625, 48000., 96000.] {
            let mut model = Model::default();
            for n in 0..8192 {
                let input = if n % 127 == 0 { 2. } else { 0. };
                let output = model.sample(
                    input,
                    mode,
                    if n < 4096 { 20. } else { 20000. },
                    1.,
                    1.,
                    n as f64 / 8192.,
                    rate,
                );
                assert!(
                    output.is_finite() && output.abs() <= 4.,
                    "mode {mode}, rate {rate}, frame {n}: {output}"
                );
            }
        }
    }
}

#[test]
fn second_filter_and_parallel_routing_change_actual_audio() {
    let mut base = Engine::new(48000.);
    base.set_param(1, FILTER_MODEL, 1.);
    base.set_param(1, CUTOFF, 2400.);
    let original = audio(base, false);
    let mut serial = Engine::new(48000.);
    serial.set_param(1, FILTER_MODEL, 1.);
    serial.set_param(1, CUTOFF, 2400.);
    serial.set_param(1, FILTER2, 20.);
    serial.set_param(1, CUTOFF2, 3800.);
    let filtered = audio(serial, false);
    let mut parallel = Engine::new(48000.);
    parallel.set_param(1, FILTER_MODEL, 1.);
    parallel.set_param(1, CUTOFF, 2400.);
    parallel.set_param(1, FILTER2, 20.);
    parallel.set_param(1, CUTOFF2, 3800.);
    parallel.set_param(1, FILTER_ROUTING, 1.);
    parallel.set_param(1, FILTER_BALANCE, 0.5);
    let mixed = audio(parallel, false);
    assert_ne!(original, filtered);
    assert_ne!(filtered, mixed);
    assert!(mixed.iter().any(|v| v.abs() > 0.001));
}

#[test]
fn one_voices_parameter_and_envelope_can_modulate_another_voice() {
    let mut low = Engine::new(48000.);
    low.set_route(0, 12, 1, PITCH as u8, 0.2);
    low.set_route_source(0, 0, PAN as u8, true);
    low.set_param(0, PAN, -1.);
    let low = audio(low, false);
    let mut high = Engine::new(48000.);
    high.set_route(0, 12, 1, PITCH as u8, 0.2);
    high.set_route_source(0, 0, PAN as u8, true);
    high.set_param(0, PAN, 1.);
    let saved_pitch = high.params[1][PITCH];
    let high_audio = audio(high, false);
    assert_ne!(low, high_audio);
    let mut with_envelope = Engine::new(48000.);
    with_envelope.set_route(0, 11, 1, PITCH as u8, 0.2);
    with_envelope.set_route_source(0, 0, 0, true);
    assert_eq!(with_envelope.params[1][PITCH], saved_pitch);
    let modulated = audio(with_envelope, true);
    let mut idle_source = Engine::new(48000.);
    idle_source.set_route(0, 11, 1, PITCH as u8, 0.2);
    idle_source.set_route_source(0, 0, 0, true);
    assert_ne!(modulated, audio(idle_source, false));
}

#[test]
fn track_swing_adds_to_global_swing_without_moving_other_tracks() {
    let mut e = Engine::new(48000.);
    e.patterns = lxr02::sequencer::blank_patterns();
    e.tempo(120., 0.1);
    for t in 0..2 {
        for s in 0..3 {
            e.set_step(0, t, s, 1., 1., 0, 1);
        }
    }
    e.track_swing(0, 0, 0.2);
    e.play();
    for _ in 0..4 {
        e.render(4096);
    }
    let frames = |track| {
        e.hits()
            .iter()
            .filter(|h| h.track == track)
            .map(|h| h.frame)
            .collect::<Vec<_>>()
    };
    let a = frames(0);
    let b = frames(1);
    assert_eq!(a[0], b[0]);
    assert!((a[1] as i64 - 7800).abs() <= 1);
    assert!((b[1] as i64 - 6600).abs() <= 1);
    assert_eq!(a[2], b[2]);
}
