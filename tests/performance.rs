use lxr02::{params::*, Engine};
fn blank() -> Engine {
    let mut e = Engine::new(48000.);
    e.patterns = lxr02::sequencer::blank_patterns();
    e.tempo(120., 0.);
    e
}
fn run(e: &mut Engine, n: usize) {
    let mut left = n;
    while left > 0 {
        let n = left.min(4096);
        e.render(n);
        left -= n;
    }
}
#[test]
fn divisions_lengths_and_triplets_are_independent() {
    let mut e = blank();
    for t in 0..3 {
        e.set_length(0, t, 3);
        for s in 0..3 {
            e.set_step(0, t, s, 1., 1., 0, 1);
        }
    }
    e.track_settings(0, 0, 4, 0., 0, 0.);
    e.track_settings(0, 1, 3, 0., 0, 0.);
    e.track_settings(0, 2, 9, 0., 0, 0.);
    e.play();
    run(&mut e, 24001);
    for (t, expected) in [
        (0, vec![0, 6000, 12000, 18000, 24000]),
        (1, vec![0, 12000, 24000]),
        (2, vec![0, 4000, 8000, 12000, 16000, 20000, 24000]),
    ] {
        let actual: Vec<_> = e
            .hits()
            .iter()
            .filter(|h| h.track == t)
            .map(|h| h.frame)
            .collect();
        assert_eq!(actual, expected);
    }
    assert_eq!(e.positions[..3], [1, 2, 0]);
}
#[test]
fn microtiming_can_advance_and_delay_hits() {
    let mut e = blank();
    for s in 0..3 {
        e.set_step(0, 0, s, 1., 1., 0, 1);
    }
    e.step_extras(0, 0, 0, 0.2, 0, 2, 0);
    e.step_extras(0, 0, 1, -0.2, 0, 2, 0);
    e.play();
    run(&mut e, 12001);
    let frames: Vec<_> = e.hits().iter().map(|h| h.frame).collect();
    assert_eq!(frames, [1201, 4800, 12000]);
}
#[test]
fn conditions_follow_each_tracks_own_cycle() {
    let mut e = blank();
    e.set_length(0, 0, 1);
    e.set_step(0, 0, 0, 1., 1., 0, 1);
    e.step_extras(0, 0, 0, 0., 7, 4, 3);
    e.play();
    run(&mut e, 48000);
    let frames: Vec<_> = e.hits().iter().map(|h| h.frame).collect();
    assert_eq!(frames, [18000, 42000]);
    e.stop();
    e.clear_hits();
    e.step_extras(0, 0, 0, 0., 3, 2, 0);
    e.play();
    run(&mut e, 6000);
    assert!(e.hits().is_empty());
    e.fill = true;
    run(&mut e, 1);
    assert_eq!(e.hits().len(), 1);
}
#[test]
fn ratchets_use_the_tracks_division_and_stop_cancels_them() {
    let mut e = blank();
    e.track_settings(0, 0, 3, 0., 0, 0.);
    e.set_step(0, 0, 0, 1., 1., 0, 4);
    e.play();
    run(&mut e, 9001);
    let frames: Vec<_> = e.hits().iter().map(|h| h.frame).collect();
    assert_eq!(frames, [0, 3000, 6000, 9000]);
    e.stop();
    e.clear_hits();
    run(&mut e, 20000);
    assert!(e.hits().is_empty());
}
#[test]
fn scene_recall_keeps_shared_parameters_and_effects() {
    let mut e = blank();
    e.set_param(0, PITCH, 50.);
    e.set_fx(0, 4.);
    e.capture_scene(0, true);
    e.set_param(0, PITCH, 90.);
    e.set_fx(0, 6.);
    assert_eq!(e.scenes[0].params[0][PITCH], 90.);
    e.immediate_pattern(1);
    assert_eq!(e.fx.kind, 6);
    e.set_param(0, PITCH, 100.);
    e.set_fx(0, 2.);
    e.immediate_pattern(0);
    assert_eq!(e.fx.kind, 6);
    assert_eq!(e.params[0][PITCH], 100.);
    e.immediate_pattern(1);
    assert_eq!(e.fx.kind, 2);
}
#[test]
fn sample_layers_render_without_the_synth_and_share_hat_choke() {
    let mut e = blank();
    e.sample_data(0, 48000., vec![0.5; 4800]);
    e.sample_settings(0, 1., 0., 0., 1., 4., true);
    e.trigger(0, 1., 0);
    let audio = e.render(512);
    assert!(audio.iter().any(|x| x.abs() > 0.01));
    e.sample_data(6, 44100., vec![0.5; 1000]);
    e.trigger(6, 1., 0);
    e.trigger(5, 1., 0);
    e.panic();
    assert!(e.render(128).iter().all(|x| *x == 0.));
}
