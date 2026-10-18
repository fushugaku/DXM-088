use lxr02_plugin::{Instrument, DEFAULT_DRUM_NOTES};

fn multi(i: &mut Instrument, frames: usize) -> Vec<f32> {
    let mut out = vec![0.; frames * 16];
    i.render_multi(&mut out);
    assert!(out.iter().all(|x| x.is_finite()));
    out
}
fn peak(out: &[f32], bus: usize) -> f32 {
    out.chunks_exact(16)
        .flat_map(|f| f[bus * 2..bus * 2 + 2].iter())
        .map(|x| x.abs())
        .fold(0., f32::max)
}

#[test]
fn every_drum_note_is_one_shot_on_its_own_stereo_bus_at_all_host_rates() {
    for rate in [44100., 48000., 96000., 192000.] {
        for (track, &note) in DEFAULT_DRUM_NOTES.iter().enumerate() {
            let mut i = Instrument::new(rate);
            i.set(283, 1.);
            i.set(284, 1.);
            i.transport(120., 0., true);
            i.midi(0x9f, note, 100);
            let out = multi(&mut i, 4096);
            assert!(!i.engine.playing);
            assert_eq!(i.engine.pattern, 0);
            assert_eq!(i.engine.hits().len(), 1);
            assert_eq!(i.engine.hits()[0].track, track as u8);
            for bus in 0..8 {
                if bus == track + 1 {
                    assert!(peak(&out, bus) > 0.001, "{rate} Hz track {track}");
                } else {
                    assert_eq!(peak(&out, bus), 0., "cross-talk on bus {bus}");
                }
            }
            i.midi(0x8f, note, 0);
            multi(&mut i, 4096);
            assert_eq!(i.engine.hits().len(), 1, "held notes must not repeat");
        }
    }
}

#[test]
fn short_note_off_keeps_the_percussion_tail_and_velocity() {
    let audio = |velocity, release| {
        let mut i = Instrument::new(48000.);
        i.set(283, 1.);
        i.midi(0x90, 36, velocity);
        if release {
            i.midi(0x80, 36, 0);
        }
        let out = multi(&mut i, 4096);
        assert_eq!(i.engine.hits().len(), 1);
        assert!((i.engine.hits()[0].velocity - velocity as f32 / 127.).abs() < 1e-6);
        out
    };
    let full = audio(127, false);
    assert_eq!(full, audio(127, true));
    assert!(peak(&full, 0) > peak(&audio(40, true), 0) * 1.5);
    assert!(peak(&full, 0) > 0.001);
    for bus in 1..8 {
        assert_eq!(peak(&full, bus), 0.);
    }
}

#[test]
fn remapping_mode_switch_and_unmapped_notes_do_not_start_patterns_or_stick() {
    let mut i = Instrument::new(48000.);
    i.midi(0x90, 127, 100);
    i.set(283, 1.);
    assert_eq!(i.active_note, None);
    i.engine.clear_hits();
    i.midi(0x90, 127, 100);
    multi(&mut i, 1024);
    assert!(i.engine.hits().is_empty());
    assert!(!i.engine.playing);
    i.set(285, 127.);
    i.midi(0x90, 127, 100);
    i.midi(0x90, 127, 90);
    i.midi(0x90, 127, 0);
    assert_eq!(i.engine.hits().len(), 2);
    assert_eq!(i.engine.hits()[1].track, 0);
    i.set(283, 0.);
    assert_eq!(i.active_note, None);
    i.midi(0x90, 127, 100);
    assert_eq!(i.engine.pattern, 127);
    assert!(i.engine.playing);
    i.midi(0x80, 127, 0);
    assert!(!i.engine.playing);
}

#[test]
fn individual_outputs_bypass_shared_fx_and_respect_track_mute() {
    let stem = |fx, muted| {
        let mut i = Instrument::new(48000.);
        i.set(283, 1.);
        i.set(284, 1.);
        i.set(234, fx);
        i.set(235, 1.);
        i.set(256, muted as u8 as f32);
        // Settle the channel mute smoothing before triggering.
        multi(&mut i, 4096);
        i.midi(0x90, 36, 100);
        multi(&mut i, 4096)
    };
    assert_eq!(stem(0., false), stem(1., false));
    assert!(peak(&stem(0., false), 1) > 0.001);
    assert_eq!(peak(&stem(0., true), 1), 0.);
}

#[test]
fn preview_is_explicit_in_drum_mode_and_settings_survive_state_and_clone() {
    let mut i = Instrument::new(48000.);
    i.set(283, 1.);
    i.set(284, 1.);
    i.set(291, 127.);
    i.midi(0x90, 127, 100);
    i.command(15, [0.; 8]);
    multi(&mut i, 1024);
    assert!(i.preview && i.engine.playing);
    i.midi(0x80, 127, 0);
    assert!(i.preview);
    let mut restored = Instrument::new(96000.);
    restored.load(&i.project()).unwrap();
    for n in [&restored, &i.copy_project(96000.)] {
        assert_eq!(n.midi_mode, 1);
        assert_eq!(n.output_mode, 1);
        assert_eq!(n.drum_notes[6], 127);
        assert!(n.engine.capture_stems);
        assert!(!n.preview);
        assert!(!n.engine.playing);
    }
    let mut old = i.project();
    old["plugin"] = serde_json::json!({"version":2,"baseNote":24});
    restored.load(&old).unwrap();
    assert_eq!(restored.midi_mode, 0);
    assert_eq!(restored.output_mode, 0);
    assert_eq!(restored.base_note, 24);
    assert_eq!(restored.drum_notes, DEFAULT_DRUM_NOTES);
}

#[test]
fn pattern_mode_also_uses_separate_outputs_and_all_sound_off_stops_preview() {
    let mut i = Instrument::new(48000.);
    i.set(284, 1.);
    i.midi(0x90, 0, 127);
    let out = multi(&mut i, 48000);
    assert_eq!(peak(&out, 0), 0.);
    assert!(peak(&out, 1) > 0.001);
    assert!(peak(&out, 4) > 0.001);
    assert!(i.engine.playing);
    i.midi(0x80, 0, 0);
    assert!(!i.engine.playing);
    i.command(15, [0.; 8]);
    assert!(i.preview);
    i.midi(0xb0, 120, 0);
    assert!(!i.preview);
    let silent = multi(&mut i, 4096);
    assert!(silent.iter().all(|&x| x == 0.));
}
