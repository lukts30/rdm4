//! Gist protocol: preload group, 30 shuffled warmup sweeps, 100 measured sweeps,
//! parse + drop included, median ms per sweep.
use std::time::Instant;
fn main() {
    let group = std::env::args().nth(1).unwrap();
    let sweeps: usize = std::env::args().nth(2).map_or(130, |n| n.parse().unwrap());
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rdm4lib/rdm/benchmark");
    let groups: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("groups.json")).unwrap()).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
    let ids: Vec<u64> = groups[group.as_str()]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    let inputs: Vec<Vec<u8>> = manifest
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| ids.contains(&e["id"].as_u64().unwrap()))
        .map(|e| std::fs::read(dir.join(e["path"].as_str().unwrap())).unwrap())
        .collect();
    let mut order: Vec<usize> = (0..inputs.len()).collect();
    let mut seed = 0x9e3779b97f4a7c15u64;
    let mut shuffle = |order: &mut Vec<usize>| {
        for i in (1..order.len()).rev() {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            order.swap(i, (seed % (i as u64 + 1)) as usize);
        }
    };
    let mut samples = Vec::new();
    for sweep in 0..sweeps {
        shuffle(&mut order);
        let start = Instant::now();
        for &i in &order {
            drop(std::hint::black_box(
                rdm_core::Document::from_bytes(std::hint::black_box(&inputs[i])).unwrap(),
            ));
        }
        if sweep >= 30 {
            samples.push(start.elapsed().as_secs_f64() * 1e3);
        }
    }
    samples.sort_by(f64::total_cmp);
    if let Some(median) = samples.get(samples.len() / 2) {
        println!("{median:.3}");
    }
}
