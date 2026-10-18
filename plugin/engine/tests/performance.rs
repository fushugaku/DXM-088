use lxr02_plugin::Instrument;
fn run(i: &mut Instrument, frames: usize) {
    let mut left = frames;
    let mut out = [0.; 2048];
    while left > 0 {
        let n = left.min(1024);
        i.render(&mut out[..n * 2]);
        left -= n;
    }
}
#[test]
fn velocity_pressure_wheel_and_fill_follow_live_midi() {
    let mut i = Instrument::new(48000.);
    i.set(278, 1.);
    i.midi(0x92, 24, 64);
    assert!((i.engine.performance_velocity - 64. / 127.).abs() < 1e-6);
    i.midi(0xd2, 100, 0);
    assert!((i.engine.pressure - 100. / 127.).abs() < 1e-6);
    i.midi(0xb2, 1, 90);
    assert!((i.engine.modwheel - 90. / 127.).abs() < 1e-6);
    i.midi(0xb2, 64, 127);
    assert!(i.engine.fill);
    i.midi(0xb2, 64, 0);
    assert!(!i.engine.fill);
    i.midi(0x82, 24, 0);
    assert!(i.active_note.is_none());
}
#[test]
fn quantized_note_switch_waits_but_last_release_stops_immediately() {
    let mut i = Instrument::new(48000.);
    i.transport(120., 0., true);
    i.set(280, 2.);
    i.midi(0x90, 24, 127);
    run(&mut i, 6000);
    i.midi(0x90, 25, 127);
    assert_eq!(i.engine.pattern, 0);
    run(&mut i, 90001);
    assert_eq!(i.engine.pattern, 1);
    i.midi(0x80, 25, 0);
    i.midi(0x80, 24, 0);
    assert!(!i.engine.playing);
    assert!(i.engine.queued_pattern.is_none());
}
#[test]
fn performance_project_roundtrip_keeps_scenes_samples_and_track_clocks() {
    let mut i = Instrument::new(44100.);
    i.engine.track_settings(0, 1, 9, -0.12, 2, 0.8);
    i.engine.step_extras(0, 1, 2, -0.2, 7, 4, 3);
    i.engine.set_route(0, 5, 1, 8, 0.33);
    i.set(274, 0.42);
    i.engine.capture_scene(0, true);
    i.engine
        .sample_data(1, 48000., vec![0., 0.5, -0.5, 1., -1.]);
    i.engine.sample_settings(1, 0.6, -3., 0.1, 0.9, 0.3, true);
    let saved = i.project();
    let mut j = Instrument::new(96000.);
    j.load(&saved).unwrap();
    assert_eq!(j.project(), saved);
    assert_eq!(j.engine.patterns[0].tracks[1].division, 9);
    assert_eq!(j.engine.samples[1].pcm.len(), 5);
}
#[test]
fn old_v2_projects_keep_the_same_default_clock() {
    let i = Instrument::new(44100.);
    let mut old = i.project();
    old["version"] = serde_json::json!(2);
    for key in ["performance", "scenes", "samples"] {
        old.as_object_mut().unwrap().remove(key);
    }
    let mut j = Instrument::new(44100.);
    j.load(&old).unwrap();
    assert_eq!(j.engine.patterns[0].tracks[0].division, 4);
    assert_eq!(j.engine.velocity_depth, 0.);
}
