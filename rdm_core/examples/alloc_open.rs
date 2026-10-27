//! Allocation stats measured like the gist's four-reader comparison:
//! public read-from-path (`Document::open`, file loading included),
//! 30 warmups, 20 measured repetitions, live counts must return to zero on drop.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicBool, AtomicIsize, AtomicUsize, Ordering::Relaxed},
};

struct Counting;
static ON: AtomicBool = AtomicBool::new(false);
static CALLS: AtomicUsize = AtomicUsize::new(0);
static REALLOCS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);
static LIVE_COUNT: AtomicIsize = AtomicIsize::new(0);
static LIVE_BYTES: AtomicIsize = AtomicIsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if ON.load(Relaxed) && !ptr.is_null() {
            CALLS.fetch_add(1, Relaxed);
            BYTES.fetch_add(layout.size(), Relaxed);
            LIVE_COUNT.fetch_add(1, Relaxed);
            LIVE_BYTES.fetch_add(layout.size() as isize, Relaxed);
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ON.load(Relaxed) {
            LIVE_COUNT.fetch_sub(1, Relaxed);
            LIVE_BYTES.fetch_sub(layout.size() as isize, Relaxed);
        }
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new = unsafe { System.realloc(ptr, layout, new_size) };
        if ON.load(Relaxed) && !new.is_null() {
            REALLOCS.fetch_add(1, Relaxed);
            BYTES.fetch_add(new_size.saturating_sub(layout.size()), Relaxed);
            LIVE_BYTES.fetch_add(new_size as isize - layout.size() as isize, Relaxed);
        }
        new
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

type Stats = [usize; 5]; // calls, reallocs, bytes, retained count, retained bytes

fn measure(path: &std::path::Path) -> Stats {
    for c in [&CALLS, &REALLOCS, &BYTES] {
        c.store(0, Relaxed);
    }
    LIVE_COUNT.store(0, Relaxed);
    LIVE_BYTES.store(0, Relaxed);
    ON.store(true, Relaxed);
    let document = rdm_core::Document::open(path).unwrap();
    let stats = [
        CALLS.load(Relaxed),
        REALLOCS.load(Relaxed),
        BYTES.load(Relaxed),
        LIVE_COUNT.load(Relaxed) as usize,
        LIVE_BYTES.load(Relaxed) as usize,
    ];
    drop(document);
    ON.store(false, Relaxed);
    assert_eq!(
        (LIVE_COUNT.load(Relaxed), LIVE_BYTES.load(Relaxed)),
        (0, 0),
        "drop returns to baseline"
    );
    stats
}

fn main() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rdm4lib/rdm/benchmark");
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
    let mut common = [0usize; 5];
    let mut full = [0usize; 5];
    let mut stable = true;
    println!("| Fixture | File bytes | Allocs | Reallocs | Allocated bytes | Retained count | Retained bytes |");
    println!("|---|---:|---:|---:|---:|---:|---:|");
    for entry in manifest.as_array().unwrap() {
        let id = entry["id"].as_u64().unwrap();
        let path = dir.join(entry["path"].as_str().unwrap());
        for _ in 0..30 {
            measure(&path);
        }
        let first = measure(&path);
        for _ in 1..20 {
            stable &= measure(&path) == first;
        }
        for i in 0..5 {
            full[i] += first[i];
            if ![6, 11, 15, 20].contains(&id) {
                common[i] += first[i];
            }
        }
        let [a, r, b, rc, rb] = first;
        println!(
            "| {} | {} | {a} | {r} | {b} | {rc} | {rb} |",
            entry["path"].as_str().unwrap(),
            entry["bytes"]
        );
    }
    println!("\ncommon22: {common:?}\nfull26: {full:?}\nstable across 20 repetitions: {stable}");
}
