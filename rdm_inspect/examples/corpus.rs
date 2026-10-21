//! Batch the real inspector without rendering its TUI. Paths arrive on stdin.
use rdm_inspect::{Analysis, BlockId, EdgeDirection};
use std::io::{self, BufRead};
#[path = "support/references.rs"]
mod references;

fn main() {
    println!("file\tblocks\tedges\tforward_candidates\tback_candidates\tmulti_incoming\tunreached\twarnings\tknown_missing_incoming\tknown_multi_incoming\tknown_root_incoming\tknown_unreached\tknown_tree\tknown_error\terror");
    for line in io::stdin().lock().lines() {
        let path = line.unwrap();
        let bytes = std::fs::read(&path).unwrap();
        match Analysis::parse(bytes) {
            Ok(a) => {
                let edges: Vec<_> = a
                    .edges
                    .iter()
                    .filter(|e| e.direction == EdgeDirection::Forward)
                    .map(|e| (e.source_block, e.target_block))
                    .collect();
                let forward = edges.len();
                let candidates = references::Graph::new(a.blocks.len(), BlockId::ROOT, &edges);
                let known = match references::known_graph(&a) {
                    Ok(g) => format!(
                        "{}\t{}\t{}\t{}\t{}\t",
                        g.incoming
                            .iter()
                            .enumerate()
                            .filter(|&(i, &n)| i != g.root.index() && n == 0)
                            .count(),
                        g.incoming
                            .iter()
                            .enumerate()
                            .filter(|&(i, &n)| i != g.root.index() && n > 1)
                            .count(),
                        g.incoming[g.root.index()],
                        g.seen.iter().filter(|&&seen| !seen).count(),
                        g.is_tree()
                    ),
                    Err(error) => format!("\t\t\t\tfalse\t{error}"),
                };
                println!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t",
                    path,
                    a.blocks.len(),
                    a.edges.len(),
                    forward,
                    a.edges.len() - forward,
                    candidates.incoming.iter().filter(|&&n| n > 1).count(),
                    candidates.seen.iter().filter(|&&b| !b).count(),
                    a.warnings.len(),
                    known
                );
            }
            Err(e) => println!("{}\t0\t0\t0\t0\t0\t0\t0\t\t\t\t\tfalse\t\t{}", path, e),
        }
    }
}
