//! Reader timings and allocation counts. Instrumentation lives only here.
//!
//! cargo run --release -p rdm_core --example measure -- DIR
//!
//! Without arguments the process exits immediately, for timing startup.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::time::{Duration, Instant};

use rdm_core::Document;

struct Counting;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static REALLOCS: AtomicUsize = AtomicUsize::new(0);
static LIVE: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        LIVE.fetch_add(layout.size(), Relaxed);
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Relaxed);
        System.dealloc(ptr, layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        REALLOCS.fetch_add(1, Relaxed);
        LIVE.fetch_add(new_size, Relaxed);
        LIVE.fetch_sub(layout.size(), Relaxed);
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// Median duration of `f`, repeated for roughly 100 ms.
fn median(mut f: impl FnMut()) -> Duration {
    let start = Instant::now();
    f();
    let once = start.elapsed().max(Duration::from_nanos(100));
    let runs = (Duration::from_millis(100).as_nanos() / once.as_nanos()).clamp(5, 20_000);
    let mut samples: Vec<_> = (0..runs)
        .map(|_| {
            let start = Instant::now();
            f();
            start.elapsed()
        })
        .collect();
    samples.sort();
    samples[samples.len() / 2]
}

fn main() {
    let Some(dir) = std::env::args().nth(1) else {
        return;
    };
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rdm"))
        .collect();
    paths.sort();

    println!(
        "{:<52} {:>8} {:>9} {:>9} {:>9} {:>7} {:>8} {:>9}",
        "file", "bytes", "io µs", "read µs", "write µs", "allocs", "reallocs", "retained"
    );
    let (mut total_bytes, mut total_io, mut total_read) = (0, Duration::ZERO, Duration::ZERO);
    for path in paths {
        let io = median(|| drop(std::fs::read(&path).unwrap()));
        let bytes = std::fs::read(&path).unwrap();
        let read = median(|| drop(Document::from_bytes(&bytes).unwrap()));
        let document = Document::from_bytes(&bytes).unwrap();
        let write = median(|| drop(document.to_bytes().unwrap()));
        drop(document);

        let (allocs, reallocs, live) = (
            ALLOCS.load(Relaxed),
            REALLOCS.load(Relaxed),
            LIVE.load(Relaxed),
        );
        let document = Document::from_bytes(&bytes).unwrap();
        let allocs = ALLOCS.load(Relaxed) - allocs;
        let reallocs = REALLOCS.load(Relaxed) - reallocs;
        let retained = LIVE.load(Relaxed) - live;
        drop(document);

        let name = path.file_name().unwrap().to_string_lossy();
        println!(
            "{:<52} {:>8} {:>9.1} {:>9.1} {:>9.1} {:>7} {:>8} {:>9}",
            name,
            bytes.len(),
            io.as_secs_f64() * 1e6,
            read.as_secs_f64() * 1e6,
            write.as_secs_f64() * 1e6,
            allocs,
            reallocs,
            retained
        );
        total_bytes += bytes.len();
        total_io += io;
        total_read += read;
    }
    let throughput = total_bytes as f64 / total_read.as_secs_f64() / 1e9;
    println!(
        "total: {total_bytes} bytes, io {:.1} µs, read {:.1} µs ({throughput:.2} GB/s)",
        total_io.as_secs_f64() * 1e6,
        total_read.as_secs_f64() * 1e6
    );
}
