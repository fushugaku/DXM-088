use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
struct Count;
static TRACK: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for Count {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if TRACK.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if TRACK.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        System.realloc(p, l, n)
    }
}
#[global_allocator]
static ALLOCATOR: Count = Count;
#[test]
fn midi_transport_and_render_allocate_nothing() {
    let mut i = lxr02_plugin::Instrument::new(96000.);
    for t in 0..7 {
        i.engine.sample_data(t, 44100., vec![0.2; 1024]);
        i.engine.track_settings(0, t, 9, 0.1, 2, 0.5);
    }
    for p in 0..128 {
        i.engine.capture_scene(p, true);
    }
    for r in 0..16 {
        i.engine.set_route(r, (r % 9) as u8, (r % 6) as u8, 12, 0.1);
    }
    let mut replacement=i.copy_project(96000.);
    let mut audio = [0.; 2048];
    TRACK.store(true, Ordering::SeqCst);
    for k in 0..128 {
        i.transport(60. + k as f64 * 9., k as f64 * 0.25, true);
        i.midi(0x90, k, 100);
        i.set(234, (k % 8) as f32);
        i.render(&mut audio);
        i.midi(0x80, k, 0);
    }
    i.transport(120., 0., false);
    i.midi(0xb0, 120, 0);
    i.render(&mut audio);
    i.midi(0x90,24,127);i.render(&mut audio);
    replacement.inherit_transport(&i);replacement.engine.take_audio(&mut i.engine);replacement.render(&mut audio);
    TRACK.store(false, Ordering::SeqCst);
    assert_eq!(ALLOCS.load(Ordering::SeqCst), 0);
}
