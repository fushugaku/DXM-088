use lxr02_plugin::Instrument;
fn render(i: &mut Instrument, n: usize) {
    let mut audio = vec![0.; n * 2];
    i.render(&mut audio);
    assert!(audio.iter().all(|x| x.is_finite()));
}
#[test]
fn note_selected_pattern_edits_survive_switches_and_project_restore() {
    let mut i = Instrument::new(48000.);
    i.transport(120., 0., true);
    for p in 0..128 {
        i.engine.capture_scene(p, true);
    }
    i.midi(0x90, 36, 100);
    i.command(3, [36., 0., 11., 0., 0., 0., 0., 0.]);
    i.command(4, [36., 0., 3., 0.42, 0.75, -2., 3., 0.]);
    i.command(17, [36., 0., 3., 0.1, 2., 0.5, 0., 0.]);
    i.command(25, [36., 0., 0., 123., 0., 0., 0., 0.]);
    i.command(27, [36., 0., 5., 0., 0., 0., 0., 0.]);
    render(&mut i, 512);
    i.midi(0x80, 36, 0);
    i.midi(0x90, 45, 100);
    // An editor update queued just before a MIDI switch stays in its own slot.
    i.command(25, [36., 0., 0., 234., 0., 0., 0., 0.]);
    assert_ne!(i.engine.scenes[45].params[0][0], 234.);
    let saved = i.project();
    let mut restored = Instrument::new(96000.);
    restored.load(&saved).unwrap();
    restored.midi(0x90, 36, 100);
    render(&mut restored, 512);
    assert_eq!(restored.engine.pattern, 36);
    let track = restored.engine.patterns[36].tracks[0];
    assert_eq!(track.length, 11);
    assert_eq!(track.division, 3);
    assert_eq!(track.steps[3].velocity, 0.42);
    assert_eq!(restored.engine.scenes[36].params[0][0], 234.);
    assert_eq!(restored.engine.scenes[36].fx[0], 5.);
}

#[test]
fn user_kit_identity_is_embedded_and_needs_no_library_on_restore() {
    let mut i = Instrument::new(48000.);
    let mut project = i.project();
    project["kitPreset"] = serde_json::json!({"id":"saved-kit","name":"Мой кит"});
    project["plugin"]["patternMemory"] = serde_json::json!(true);
    i.load(&project).unwrap();
    i.set(0, 123.);
    let mut restored = Instrument::new(44100.);
    restored.load(&i.project()).unwrap();
    assert_eq!(restored.project()["kitPreset"], project["kitPreset"]);
    assert_eq!(restored.project()["plugin"]["patternMemory"], true);
    assert_eq!(restored.get(0), 123.);
    assert_eq!(i.copy_project(96000.).project(), i.project());
}

#[test]
fn solo_in_one_midi_pattern_does_not_silence_another_patterns_mix() {
    let mut i = Instrument::new(48000.);
    for p in 0..128 {
        i.engine.capture_scene(p, true);
    }
    i.midi(0x90, 36, 100);
    i.set(263, 1.);
    i.midi(0x80, 36, 0);
    i.midi(0x90, 45, 100);
    assert!(i.engine.solo[0]);
    assert!(!i.engine.scenes[45].solo.iter().any(|s| *s));
    i.engine.trigger(3, 1., 0);
    render(&mut i, 512);
    assert!(i.engine.meters[3] > 0.001);
}
