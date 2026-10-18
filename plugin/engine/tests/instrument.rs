use lxr02_plugin::Instrument;
fn render(i: &mut Instrument, n: usize) {
    let mut out = vec![0.; n * 2];
    i.render(&mut out);
    assert!(out.iter().all(|x| x.is_finite()));
}
#[test]
fn only_mapped_held_notes_gate_the_sequence() {
    let mut i = Instrument::new(48000.);
    i.transport(120., 0., true);
    render(&mut i, 512);
    assert!(!i.engine.playing);
    i.midi(0x90, 36, 100);
    assert_eq!(i.active_note, None);
    for key in 24..32 {
        i.midi(0x90, key, 100);
        assert_eq!(i.active_note, Some(key));
        assert_eq!(i.engine.pattern, (key - 24) as usize);
        render(&mut i, 1);
        assert!(i.engine.playing);
        i.midi(0x80, key, 0);
        assert!(!i.engine.playing);
    }
}
#[test]
fn last_note_priority_falls_back_across_channels() {
    let mut i = Instrument::new(44100.);
    i.midi(0x90, 24, 100);
    i.midi(0x91, 29, 127);
    i.midi(0x90, 27, 80);
    assert_eq!(i.engine.pattern, 3);
    i.midi(0x80, 27, 0);
    assert_eq!(i.active_note, Some(29));
    assert_eq!(i.engine.pattern, 5);
    i.midi(0x81, 29, 0);
    assert_eq!(i.active_note, Some(24));
    i.midi(0x90, 24, 0);
    assert_eq!(i.active_note, None);
}
#[test]
fn duplicate_keys_and_all_notes_off_do_not_stick() {
    let mut i = Instrument::new(48000.);
    i.midi(0x90, 24, 100);
    i.midi(0x90, 24, 80);
    i.midi(0x80, 24, 0);
    assert_eq!(i.active_note, Some(24));
    i.midi(0x91, 25, 100);
    i.midi(0xb0, 123, 0);
    assert_eq!(i.active_note, Some(25));
    i.midi(0xb1, 120, 0);
    assert_eq!(i.active_note, None);
}
#[test]
fn new_mapping_clears_held_keys() {
    let mut i = Instrument::new(48000.);
    i.midi(0x90, 24, 127);
    i.set(273, 120.);
    assert_eq!(i.active_note, None);
    i.midi(0x90, 127, 127);
    assert_eq!(i.engine.pattern, 7);
    i.midi(0x80, 127, 0);
    assert_eq!(i.active_note, None);
}
#[test]
fn host_grid_swing_tempo_and_seek() {
    let mut i = Instrument::new(48000.);
    i.set(270, 0.5);
    i.transport(120., 0., true);
    i.midi(0x90, 24, 127);
    render(&mut i, 9000);
    assert_eq!(i.engine.positions[0], 0);
    render(&mut i, 1);
    assert_eq!(i.engine.positions[0], 1);
    i.transport(240., 4.5, true);
    i.engine.set_length(0, 1, 5);
    render(&mut i, 1);
    assert_eq!(i.engine.positions[0], 2);
    assert_eq!(i.engine.positions[1], 3);
    i.transport(60., 0., true);
    render(&mut i, 1);
    assert_eq!(i.engine.positions[0], 0);
    i.transport(60., 0., false);
    assert_eq!(i.active_note, None);
    assert!(!i.engine.playing);
}
#[test]
fn live_notes_work_with_stopped_host() {
    let mut i = Instrument::new(48000.);
    i.transport(120., 99., false);
    i.midi(0x90, 24, 127);
    render(&mut i, 6001);
    assert_eq!(i.engine.positions[0], 1);
    i.midi(0x80, 24, 0);
    assert!(!i.engine.playing);
    i.midi(0x90, 25, 127);
    render(&mut i, 1);
    assert_eq!(i.engine.positions[0], 0);
}
#[test]
fn changing_notes_uses_current_grid_immediately() {
    let mut i = Instrument::new(48000.);
    i.transport(120., 2.25, true);
    i.midi(0x90, 24, 127);
    render(&mut i, 128);
    i.midi(0x90, 31, 127);
    render(&mut i, 1);
    assert_eq!(i.engine.pattern, 7);
    assert_eq!(i.engine.positions[0], 9);
}
#[test]
fn block_size_does_not_change_audio() {
    fn audio(block: usize) -> Vec<f32> {
        let mut i = Instrument::new(48000.);
        i.set(270, 0.23);
        i.midi(0x90, 24, 127);
        let mut result = Vec::new();
        let mut at = 0;
        while at < 48000 {
            let n = block.min(48000 - at);
            i.transport(137., at as f64 / 48000. * 137. / 60., true);
            let mut out = vec![0.; n * 2];
            i.render(&mut out);
            result.extend(out);
            at += n;
        }
        result
    }
    assert_eq!(audio(17), audio(4096));
}
#[test]
fn project_roundtrip_keeps_all_patterns_locks_and_mapping() {
    let mut i = Instrument::new(48000.);
    i.engine.set_param(3, 8, 0.789);
    i.engine.set_step(7, 6, 63, 0.89, 0.45, -17, 4);
    i.engine.set_lock(7, 6, 63, 1, 12, 1234.);
    i.engine.set_length(7, 6, 64);
    i.set(273, 60.);
    i.set(234, 5.);
    i.set(237, 0.71);
    let saved = i.project();
    let mut other = Instrument::new(96000.);
    assert!(other.load(&saved).is_some());
    assert_eq!(other.project(), saved);
    assert_eq!(other.active_note, None);
    let mut invalid = saved.clone();
    invalid["patterns"][7]["tracks"][6][63]["locks"][1][1] = serde_json::json!(99999999);
    assert!(other.load(&invalid).is_none());
    assert_eq!(other.project(), saved);
    let clone = i.copy_project(44100.);
    assert_eq!(clone.project(), saved);
}
#[test]
fn instances_are_independent() {
    let mut a = Instrument::new(44100.);
    let b = Instrument::new(44100.);
    a.midi(0x90, 31, 127);
    a.set(8, 0.9);
    assert_eq!(b.active_note, None);
    assert_eq!(b.engine.pattern, 0);
    assert_ne!(a.get(8), b.get(8));
}

#[test]
fn project_replacement_keeps_the_current_held_note_and_grid() {
    let mut old = Instrument::new(48000.);
    old.transport(120., 3., true);
    old.midi(0x90, 27, 100);
    render(&mut old, 128);
    let mut new = old.copy_project(48000.);
    new.engine.set_step(3, 0, 12, 0.9, 1., 0, 1);
    new.inherit_transport(&old);
    assert_eq!(new.active_note, Some(27));
    assert_eq!(new.engine.pattern, 3);
    new.transport(120., 3. + 128. / 24000., true);
    render(&mut new, 1);
    assert_eq!(new.engine.positions[0], 12);
    new.midi(0x80, 27, 0);
    assert_eq!(new.active_note, None);
}
