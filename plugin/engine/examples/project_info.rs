use lxr02_plugin::Instrument;
fn main() {
    let path = std::env::args().nth(1).expect("project JSON path");
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut i = Instrument::new(48000.);
    i.load(&json).expect("invalid LXR02 project");
    println!(
        "Project v{}: {} scenes, {} samples, {} active modulation routes",
        json["version"],
        i.engine.scenes.iter().filter(|s| s.enabled).count(),
        i.engine
            .samples
            .iter()
            .filter(|s| !s.pcm.is_empty())
            .count(),
        i.engine.routes.iter().filter(|r| r.amount != 0.).count()
    );
}
