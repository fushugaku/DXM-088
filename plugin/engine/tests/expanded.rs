use lxr02::params::*;
use lxr02_plugin::{voice_parameter, Instrument, PARAMETERS};

#[test]
fn appended_automation_preserves_every_legacy_id_and_restores_extended_sound() {
    assert_eq!(PARAMETERS, 394);
    assert_eq!(lxr02_plugin::range(14), (0., 7., true));
    for p in 0..234 {
        assert_eq!(voice_parameter(p), Some((p / 39, p % 39)));
    }
    for p in 234..292 {
        assert_eq!(voice_parameter(p), None);
    }
    let mut i = Instrument::new(48000.);
    for voice in 0..6 {
        for param in 39..PARAMS {
            let id = 292 + voice * (PARAMS - 39) + param - 39;
            assert_eq!(voice_parameter(id), Some((voice, param)));
            i.set(id, MAX[param]);
            assert_eq!(i.engine.params[voice][param], MAX[param]);
        }
    }
    i.engine.track_swing(36, 3, 0.2);
    i.engine.set_route(7, 12, 3, CUTOFF2 as u8, -0.3);
    i.engine.set_route_source(7, 1, PITCH as u8, true);
    i.engine.capture_scene(45, true);
    let project = i.project();
    let mut restored = Instrument::new(96000.);
    restored.load(&project).unwrap();
    assert_eq!(restored.project(), project);
    assert_eq!(restored.copy_project(44100.).project(), project);
    assert_eq!(restored.engine.patterns[36].tracks[3].swing, 0.2);
    assert_eq!(restored.engine.routes[7].source_voice, 1);
    assert_eq!(restored.engine.routes[7].source_param, PITCH as u8);
}

#[test]
fn old_projects_gain_neutral_filters_envelopes_and_track_swing() {
    let mut old = Instrument::new(48000.).project();
    for row in old["params"].as_array_mut().unwrap() {
        row.as_array_mut().unwrap().truncate(39);
    }
    for p in old["patterns"].as_array_mut().unwrap() {
        p.as_object_mut().unwrap().remove("swings");
    }
    for route in old["performance"]["routes"].as_array_mut().unwrap() {
        for key in ["sourceVoice", "sourceParam", "configured"] {
            route.as_object_mut().unwrap().remove(key);
        }
    }
    let mut restored = Instrument::new(48000.);
    restored.load(&old).unwrap();
    assert_eq!(restored.engine.params[0][FILTER2], 0.);
    assert_eq!(restored.engine.params[0][FILTER_ENV_AMOUNT], 0.);
    assert_eq!(restored.engine.patterns[0].tracks[0].swing, 0.);
    assert_eq!(
        restored.engine.params[0][..39],
        old["params"][0]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_f64().unwrap() as f32)
            .collect::<Vec<_>>()
    );
}
