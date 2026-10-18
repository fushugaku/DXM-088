use lxr02_plugin::Instrument;

fn instrument() -> Instrument {
    let mut i = Instrument::new(48000.);
    i.engine.patterns = lxr02::sequencer::blank_patterns();
    i.engine.set_step(36, 0, 0, 1., 1., 0, 1);
    i
}

#[test]
fn held_pattern_does_not_replay_first_hit_after_a_host_loop_inside_a_block() {
    for block in [127, 512, 2048] {
        let mut i = instrument();
        i.transport(120., 0., true);
        i.midi(0x90, 36, 127);
        let mut frame = 0;
        while frame < 288000 {
            let n = block.min(288000 - frame);
            i.transport(120., (frame as f64 / 24000.) % 4., true);
            i.render(&mut vec![0.; n * 2]);
            frame += n;
        }
        let hits: Vec<_> = i
            .engine
            .hits()
            .iter()
            .filter(|h| h.track == 0)
            .map(|h| h.frame)
            .collect();
        assert_eq!(hits.len(), 3, "block {block}: {hits:?}");
        for (actual, expected) in hits.iter().zip([0u64, 96000, 192000]) {
            assert!(
                (*actual as i64 - expected as i64).abs() <= 1,
                "block {block}: {hits:?}"
            );
        }
    }
}

#[test]
fn a_continuously_held_note_has_one_first_hit_per_internal_pattern_cycle() {
    let mut i = instrument();
    i.midi(0x90, 36, 127);
    let mut frame = 0;
    while frame < 288000 {
        let n = 512.min(288000 - frame);
        i.render(&mut vec![0.; n * 2]);
        frame += n;
    }
    let hits: Vec<_> = i.engine.hits().iter().map(|h| h.frame).collect();
    // The stopped host uses the instrument's saved 128 BPM.
    assert_eq!(hits.len(), 4, "{hits:?}");
    for pair in hits.windows(2) {
        assert!(
            (pair[1] as i64 - pair[0] as i64 - 90000).abs() <= 1,
            "{hits:?}"
        );
    }
}

#[test]
fn host_loop_keeps_inflight_ratchets_and_new_note_still_starts_immediately() {
    let mut i = instrument();
    i.engine.set_step(36, 0, 0, 1., 1., 0, 3);
    i.engine.set_step(45, 0, 0, 1., 1., 0, 1);
    i.engine.set_step(45, 0, 1, 1., 1., 0, 1);
    i.transport(120., 0., true);
    i.midi(0x90, 36, 127);
    let mut frame = 0;
    while frame < 102000 {
        let n = 127.min(102000 - frame);
        i.transport(120., (frame as f64 / 24000.) % 4., true);
        i.render(&mut vec![0.; n * 2]);
        frame += n;
    }
    let hits: Vec<_> = i.engine.hits().iter().map(|h| h.frame).collect();
    assert_eq!(hits.len(), 6, "{hits:?}");
    assert!(
        hits.iter().any(|h| (*h as i64 - 98000).abs() <= 2),
        "{hits:?}"
    );
    assert!(
        hits.iter().any(|h| (*h as i64 - 100000).abs() <= 2),
        "{hits:?}"
    );
    i.midi(0x90, 45, 127);
    i.render(&mut [0.; 2]);
    assert_eq!(i.engine.hits().last().unwrap().frame, 102000);
}
