//! A project musicians can open directly in the web editor or any plugin format.
use lxr02_plugin::Instrument;
fn main() {
    let mut i = Instrument::new(44100.);
    i.engine.tempo(122., 0.08);
    for p in 0..8 {
        i.engine.immediate_pattern(p);
        i.engine.load_kit(p % 4);
        for t in 0..7 {
            let (division, length, hits) = match t {
                0 => (4, 16, 4),
                1 => (3, 13, 5),
                2 => (9, 12, 5),
                3 => (4, 16, 2),
                4 => (4, 15, 3),
                5 => (4, 16, 9),
                _ => (3, 8, 2),
            };
            i.engine.generate(p, t, hits, length, p % 3);
            i.engine.track_settings(
                p,
                t,
                division,
                if t == 3 { 0.04 } else { 0. },
                if t == 5 { 2 } else { 0 },
                0.5,
            );
        }
        i.engine.step_extras(p, 5, 14, -0.12, 7, 4, 3);
        i.engine.set_step(p, 5, 14, 0.8, 1., 0, 3);
        i.engine.capture_scene(p, true);
    }
    i.engine.set_route(0, 0, 1, 8, 0.35);
    i.engine.set_route(1, 0, 5, 12, 0.2);
    i.engine.set_route(2, 5, 0, 15, 0.4);
    i.engine.set_route(3, 6, 4, 12, 0.3);
    i.set(278, 0.5);
    i.set(280, 2.);
    i.engine.immediate_pattern(0);
    let mut j = i.project();
    j["name"] = serde_json::json!("Polyrhythm Scenes");
    j["performance"]["macroNames"] = serde_json::json!(["Metal", "Space", "Body", "Motion"]);
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "artifacts/polyrhythm-scenes.lxr02.json".into());
    std::fs::write(path, serde_json::to_string_pretty(&j).unwrap()).unwrap();
}
