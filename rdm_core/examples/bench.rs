//! Reader benchmark over `rdm4lib/rdm/benchmark`. Benchmark tooling only.
//!
//! `cargo run --release -p rdm_core --example bench -- [--rounds N]`
//!
//! Instrumentation: a global allocator wrapping `System` counts, while
//! enabled, calls to `alloc`/`alloc_zeroed` (allocations), calls to
//! `realloc` (reallocations), requested bytes (`alloc` sizes plus the growth
//! of growing reallocs) and live requested bytes. Allocator rounding and
//! allocator-internal metadata are not visible to it. Counting is enabled
//! only around `Document::from_bytes` in the allocation pass, never while
//! timing.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed},
    time::{Duration, Instant},
};

use rdm_core::Document;

struct CountingAllocator;

static COUNTING: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static REALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static REQUESTED: AtomicUsize = AtomicUsize::new(0);
static LIVE: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count_allocation(layout.size());
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count_allocation(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if COUNTING.load(Relaxed) {
            LIVE.fetch_sub(layout.size(), Relaxed);
        }
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if COUNTING.load(Relaxed) {
            REALLOCATIONS.fetch_add(1, Relaxed);
            if new_size > layout.size() {
                REQUESTED.fetch_add(new_size - layout.size(), Relaxed);
                LIVE.fetch_add(new_size - layout.size(), Relaxed);
            } else {
                LIVE.fetch_sub(layout.size() - new_size, Relaxed);
            }
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

fn count_allocation(size: usize) {
    if COUNTING.load(Relaxed) {
        ALLOCATIONS.fetch_add(1, Relaxed);
        REQUESTED.fetch_add(size, Relaxed);
        LIVE.fetch_add(size, Relaxed);
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Allocations {
    allocations: usize,
    reallocations: usize,
    requested: usize,
    retained: usize,
}

fn measure_allocations(read: impl FnOnce() -> Document) -> Allocations {
    for counter in [&ALLOCATIONS, &REALLOCATIONS, &REQUESTED, &LIVE] {
        counter.store(0, Relaxed);
    }
    COUNTING.store(true, Relaxed);
    let document = read();
    COUNTING.store(false, Relaxed);
    let result = Allocations {
        allocations: ALLOCATIONS.load(Relaxed),
        reallocations: REALLOCATIONS.load(Relaxed),
        requested: REQUESTED.load(Relaxed),
        retained: LIVE.load(Relaxed),
    };
    drop(document);
    result
}

/// Median of repeated runs, taking at least 0.2 s or 15 runs.
fn median_time(mut run: impl FnMut()) -> Duration {
    let mut samples = Vec::new();
    let start = Instant::now();
    while samples.len() < 15
        || (start.elapsed() < Duration::from_millis(200) && samples.len() < 5000)
    {
        let begin = Instant::now();
        run();
        samples.push(begin.elapsed());
    }
    samples.sort_unstable();
    samples[samples.len() / 2]
}

struct Fixture {
    id: u64,
    path: PathBuf,
    animation: bool,
}

fn fixtures(dir: &Path) -> Vec<Fixture> {
    let groups: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dir.join("groups.json")).unwrap()).unwrap();
    let animation: Vec<u64> = groups["animation"]
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_u64().unwrap())
        .collect();
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
    manifest
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            let id = entry["id"].as_u64().unwrap();
            Fixture {
                id,
                path: dir.join(entry["path"].as_str().unwrap()),
                animation: animation.contains(&id),
            }
        })
        .collect()
}

fn micros(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1e6
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--noop") {
        return;
    }
    let rounds: usize = args
        .iter()
        .position(|a| a == "--rounds")
        .map_or(3, |i| args[i + 1].parse().unwrap());

    let startup = median_time(|| {
        let status = Command::new(&args[0]).arg("--noop").status().unwrap();
        assert!(status.success());
    });
    println!(
        "process startup (spawn + exit of this binary with --noop, median): {:.0} us\n",
        micros(startup)
    );

    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rdm4lib/rdm/benchmark");
    let fixtures = fixtures(&dir);
    let mut totals = Vec::new();
    let mut allocations_by_round: Vec<Vec<Allocations>> = Vec::new();

    for round in 0..rounds {
        let mut round_allocations = Vec::new();
        let mut total = [[0.0f64; 3]; 2]; // [group][bytes, io us, parse us]
        if round == 0 {
            println!("| id | group | bytes | I/O us | parse us | parse MB/s | allocs | reallocs | requested B | retained B |");
            println!("|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|");
        }
        for fixture in &fixtures {
            let input = fs::read(&fixture.path).unwrap();
            let io = median_time(|| {
                std::hint::black_box(fs::read(&fixture.path).unwrap());
            });
            let parse = median_time(|| {
                let document = Document::from_bytes(std::hint::black_box(&input)).unwrap();
                std::hint::black_box(&document);
            });
            let allocations = measure_allocations(|| Document::from_bytes(&input).unwrap());
            round_allocations.push(allocations);
            // `open` reads the file itself; it must not retain the input buffer.
            let opened = measure_allocations(|| Document::open(&fixture.path).unwrap());
            assert_eq!(
                opened.retained,
                allocations.retained,
                "{}",
                fixture.path.display()
            );

            let group = usize::from(fixture.animation);
            total[group][0] += input.len() as f64;
            total[group][1] += micros(io);
            total[group][2] += micros(parse);
            if round == 0 {
                println!(
                    "| {} | {} | {} | {:.1} | {:.1} | {:.0} | {} | {} | {} | {} |",
                    fixture.id,
                    if fixture.animation {
                        "anim"
                    } else {
                        "non-anim"
                    },
                    input.len(),
                    micros(io),
                    micros(parse),
                    input.len() as f64 / micros(parse),
                    allocations.allocations,
                    allocations.reallocations,
                    allocations.requested,
                    allocations.retained,
                );
            }
        }
        totals.push(total);
        allocations_by_round.push(round_allocations);
    }

    println!("\n| round | group | bytes | sum I/O us | sum parse us | parse MB/s |");
    println!("|---:|---|---:|---:|---:|---:|");
    for (round, total) in totals.iter().enumerate() {
        for (group, name) in [(1, "animation"), (0, "non-animation")] {
            let [bytes, io, parse] = total[group];
            println!(
                "| {round} | {name} | {bytes} | {io:.0} | {parse:.0} | {:.0} |",
                bytes / parse
            );
        }
    }

    let stable = allocations_by_round
        .windows(2)
        .all(|pair| pair[0] == pair[1]);
    println!("\nallocation counts identical across {rounds} rounds: {stable}");
    println!("Document::open retains the same bytes as from_bytes for every file (asserted)");
    for (group, name) in [(true, "animation"), (false, "non-animation")] {
        let mut sum = [0usize; 4];
        for (fixture, a) in fixtures.iter().zip(&allocations_by_round[0]) {
            if fixture.animation == group {
                sum[0] += a.allocations;
                sum[1] += a.reallocations;
                sum[2] += a.requested;
                sum[3] += a.retained;
            }
        }
        println!(
            "{name}: allocs {} reallocs {} requested {} B retained {} B",
            sum[0], sum[1], sum[2], sum[3]
        );
    }
}
