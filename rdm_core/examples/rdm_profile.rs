//! Measures reading: file I/O, the first (cold) parse, steady-state parses,
//! allocations and the bytes a document retains.
//!
//! Usage: cargo run --release -p rdm_core --example rdm_profile -- [--iterations N] FILE...

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::time::{Duration, Instant};

use rdm_core::Document;

struct Counting;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static REALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
static LIVE: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Relaxed);
        ALLOCATED.fetch_add(layout.size(), Relaxed);
        LIVE.fetch_add(layout.size(), Relaxed);
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Relaxed);
        System.dealloc(ptr, layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        REALLOCATIONS.fetch_add(1, Relaxed);
        ALLOCATED.fetch_add(new_size.saturating_sub(layout.size()), Relaxed);
        LIVE.fetch_add(new_size, Relaxed);
        LIVE.fetch_sub(layout.size(), Relaxed);
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

#[derive(Clone, Copy)]
struct Counters {
    allocations: usize,
    reallocations: usize,
    allocated: usize,
    live: usize,
}

fn counters() -> Counters {
    Counters {
        allocations: ALLOCATIONS.load(Relaxed),
        reallocations: REALLOCATIONS.load(Relaxed),
        allocated: ALLOCATED.load(Relaxed),
        live: LIVE.load(Relaxed),
    }
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    samples[samples.len() / 2]
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut iterations = 50;
    if args.first().is_some_and(|arg| arg == "--iterations") {
        iterations = args[1].parse().expect("iteration count");
        args.drain(..2);
    }

    println!(
        "{:<52} {:>9} {:>9} {:>9} {:>10} {:>9} {:>7} {:>7} {:>10} {:>10}",
        "file",
        "bytes",
        "io",
        "first",
        "read(med)",
        "MB/s",
        "allocs",
        "reallocs",
        "allocated",
        "retained"
    );
    for path in &args {
        let start = Instant::now();
        let bytes = std::fs::read(path).unwrap();
        let io = start.elapsed();

        let start = Instant::now();
        let first = Document::from_bytes(&bytes).unwrap();
        let first_read = start.elapsed();
        drop(first);

        let mut samples = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            let start = Instant::now();
            let document = Document::from_bytes(&bytes).unwrap();
            samples.push(start.elapsed());
            drop(document);
        }
        let read = median(samples);

        let before = counters();
        let document = Document::from_bytes(&bytes).unwrap();
        let after = counters();
        assert_eq!(document.to_bytes().unwrap(), bytes);

        let name = path.rsplit('/').next().unwrap();
        let throughput = bytes.len() as f64 / read.as_secs_f64() / 1e6;
        println!(
            "{:<52} {:>9} {:>9.1?} {:>9.1?} {:>10.1?} {:>9.1} {:>7} {:>7} {:>10} {:>10}",
            name,
            bytes.len(),
            io,
            first_read,
            read,
            throughput,
            after.allocations - before.allocations,
            after.reallocations - before.reallocations,
            after.allocated - before.allocated,
            after.live - before.live,
        );
    }
}
