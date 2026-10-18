use lxr02::Engine;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
thread_local! {static TRACKING:Cell<bool>=const {Cell::new(false)};static ALLOCATIONS:Cell<usize>=const {Cell::new(0)};}
struct Allocator;
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        TRACKING.with(|active| {
            if active.get() {
                ALLOCATIONS.with(|count| count.set(count.get() + 1));
            }
        });
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        System.dealloc(pointer, layout);
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        TRACKING.with(|active| {
            if active.get() {
                ALLOCATIONS.with(|count| count.set(count.get() + 1));
            }
        });
        System.realloc(pointer, layout, size)
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
#[test]
fn render_and_effect_switching_never_allocate() {
    let mut e = Engine::new(48000.);
    e.play();
    TRACKING.with(|active| active.set(true));
    for kind in [0, 1, 2, 3, 4, 5, 6, 7, 5, 4, 0] {
        e.set_fx(0, kind as f32);
        e.render(4096);
    }
    TRACKING.with(|active| active.set(false));
    assert_eq!(
        ALLOCATIONS.with(|count| count.get()),
        0,
        "real-time DSP allocated or grew its buffers"
    );
}
