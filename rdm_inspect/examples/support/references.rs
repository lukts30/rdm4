use rdm_inspect::{Analysis, BlockId, Endian};
use std::collections::HashMap;

pub struct Graph {
    pub incoming: Vec<usize>,
    pub seen: Vec<bool>,
    pub root: BlockId,
}

impl Graph {
    pub fn new(blocks: usize, root: BlockId, edges: &[(BlockId, BlockId)]) -> Self {
        let mut incoming = vec![0; blocks];
        let mut children = vec![Vec::new(); blocks];
        // Count every reference, including duplicate references to a visited target.
        for &(source, target) in edges {
            incoming[target.index()] += 1;
            children[source.index()].push(target);
        }
        // Iterative DFS: pop() visits each adjacency list in reverse insertion order.
        // known_graph() preserves record/field order within each source's edge list,
        // so this repeats its reverse sibling order; the two reversals do not cancel.
        // `seen` prevents repeated expansion of shared targets or cycles. This checks
        // reachability in O(blocks + edges), not physical depth-first order in the file.
        let mut seen = vec![false; blocks];
        let mut todo = vec![root];
        while let Some(block) = todo.pop() {
            if !seen[block.index()] {
                seen[block.index()] = true;
                todo.extend(&children[block.index()]);
            }
        }
        Self {
            incoming,
            seen,
            root,
        }
    }

    pub fn exactly_one_per_non_root(&self) -> bool {
        self.incoming
            .iter()
            .enumerate()
            .all(|(block, &count)| count == if block == self.root.index() { 0 } else { 1 })
    }

    pub fn is_tree(&self) -> bool {
        self.exactly_one_per_non_root() && self.seen.iter().all(|&seen| seen)
    }
}

// Recovered record layouts, independent of the inspector's byte-value matches.
fn layout(kind: &str) -> (&[u32], &[(usize, &str)]) {
    match kind {
        "root" => (
            &[48, 52],
            &[
                (0, "export"),
                (4, "model"),
                (8, "material_link"),
                (12, "skeleton"),
                (16, "animation"),
                (20, "root_vertices"),
            ],
        ),
        "export" => (&[72], &[(0, "string"), (4, "string")]),
        "model" => (
            &[68, 92],
            &[
                (0, "name"),
                (4, "vertex_link"),
                (8, "index_format"),
                (12, "vertices"),
                (16, "indices"),
                (20, "tri_groups"),
            ],
        ),
        "name" => (&[28], &[(0, "string")]),
        "vertex_link" => (&[24], &[(0, "components")]),
        "material_link" => (&[28], &[(0, "material")]),
        "material" => (&[48], &[(0, "string"), (4, "string")]),
        "skeleton" => (&[32], &[(0, "bones"), (8, "companion")]),
        "bones" => (&[84], &[(0, "string")]),
        "animation" => (&[48], &[(0, "string"), (4, "tracks")]),
        "tracks" => (&[24], &[(0, "string"), (4, "frames")]),
        "root_vertices" => (&[64], &[(0, "vertex_link"), (8, "vertices")]),
        "components" => (&[16], &[]),
        "index_format" => (&[20], &[]),
        "tri_groups" => (&[28], &[]),
        "frames" => (&[32], &[]),
        "string" | "companion" => (&[1], &[]),
        "vertices" | "indices" => (&[], &[]),
        _ => unreachable!("unknown schema kind"),
    }
}

pub fn known_graph(a: &Analysis) -> Result<Graph, String> {
    let targets: HashMap<_, _> = a.blocks.iter().map(|b| (b.payload_offset, b.id)).collect();
    let root = *targets
        .get(&(a.preamble.root_payload_offset as usize))
        .ok_or("root is not a payload boundary")?;
    let mut types = vec![None; a.blocks.len()];
    // LIFO DFS: targets are pushed in record order, then layout() field order.
    // The last record's last non-null pointer is therefore explored first.
    // Edges are recorded when pushed, so each source's edge list still retains
    // record/field order for the separate reachability pass in Graph::new().
    let mut todo = vec![(root, "root")];
    let mut edges = Vec::new();
    while let Some((source, kind)) = todo.pop() {
        if let Some(previous) = types[source.index()] {
            if previous != kind {
                return Err(format!("block {source}: conflicting record types"));
            }
            continue;
        }
        types[source.index()] = Some(kind);
        let block = &a.blocks[source.index()];
        let (widths, fields) = layout(kind);
        if !widths.is_empty() && !widths.contains(&block.stride) {
            return Err(format!(
                "block {source}: unsupported {kind} stride {}",
                block.stride
            ));
        }
        for record in 0..block.count as usize {
            for &(offset, target_kind) in fields {
                let at = block.payload_offset + record * block.stride as usize + offset;
                let bytes = a.bytes[at..at + 4].try_into().unwrap();
                let value = match a.endian {
                    Endian::Little => u32::from_le_bytes(bytes),
                    Endian::Big => u32::from_be_bytes(bytes),
                } as usize;
                if value == 0 {
                    continue;
                }
                let target = *targets
                    .get(&value)
                    .ok_or_else(|| format!("pointer at {at:#x}: invalid target {value:#x}"))?;
                // Record the edge BEFORE checking whether its target was visited.
                edges.push((source, target));
                todo.push((target, target_kind));
            }
        }
    }
    Ok(Graph::new(a.blocks.len(), root, &edges))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph(blocks: usize, edges: &[(usize, usize)]) -> Graph {
        let edges: Vec<_> = edges
            .iter()
            .map(|&(s, t)| (BlockId::new(s), BlockId::new(t)))
            .collect();
        Graph::new(blocks, BlockId::ROOT, &edges)
    }

    #[test]
    fn counts_duplicate_references_even_when_every_block_is_reachable() {
        let g = graph(3, &[(0, 1), (0, 2), (1, 2)]);
        assert_eq!(g.incoming, [0, 1, 2]);
        assert!(g.seen.iter().all(|&seen| seen));
        assert!(!g.exactly_one_per_non_root());
        assert!(!g.is_tree());
    }

    #[test]
    fn incoming_counts_do_not_hide_a_disconnected_cycle() {
        let g = graph(3, &[(1, 2), (2, 1)]);
        assert!(g.exactly_one_per_non_root());
        assert!(!g.is_tree());
        assert!(!graph(2, &[]).is_tree());
        assert!(!graph(2, &[(0, 1), (1, 0)]).is_tree());
    }

    #[test]
    fn schema_traversal_keeps_two_pointers_to_the_same_payload() {
        for shared in [false, true] {
            let mut bytes = b"RDM\x01".to_vec();
            for word in [20u32, 0, 4, 28, 1, 48] {
                bytes.extend(word.to_le_bytes());
            }
            for word in [84u32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] {
                bytes.extend(word.to_le_bytes());
            }
            for word in [1u32, 72, 164, if shared { 164 } else { 0 }] {
                bytes.extend(word.to_le_bytes());
            }
            bytes.extend([0; 64]);
            for word in [3u32, 1] {
                bytes.extend(word.to_le_bytes());
            }
            bytes.extend(b"abc");
            let a = Analysis::parse(bytes).unwrap();
            let g = known_graph(&a).unwrap();
            assert_eq!(g.incoming, [0, 1, if shared { 2 } else { 1 }]);
            assert!(g.seen.iter().all(|&seen| seen));
            assert_eq!(g.is_tree(), !shared);
        }
    }
}
