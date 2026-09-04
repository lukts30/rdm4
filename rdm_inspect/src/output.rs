//! Non-interactive text and Graphviz renderers.

use std::fmt::Write;

use crate::{Analysis, Block, Edge, EdgeConfidence};

const MEANING_WIDTH: usize = 30;
const TABLE_HEADER: &str =
    "Block  Header      Payload     Meaning                         Count     Stride      Bytes  In Out";
const TABLE_RULE: &str =
    "---------------------------------------------------------------------------------------------------";

fn paint(enabled: bool, code: u8, value: impl AsRef<str>) -> String {
    if enabled {
        format!("\x1b[{code}m{}\x1b[0m", value.as_ref())
    } else {
        value.as_ref().to_owned()
    }
}

/// Render a complete structural report suitable for a terminal or log file.
pub fn summary(analysis: &Analysis, color: bool) -> String {
    let strong_edges = analysis
        .edges
        .iter()
        .filter(|edge| edge.confidence == EdgeConfidence::Strong)
        .count();
    let candidate_edges = analysis.edges.len() - strong_edges;
    let reachable = analysis.strongly_reachable();
    let reachable_count = reachable.iter().filter(|&&value| value).count();
    let mut output = String::new();
    writeln!(output, "{}", paint(color, 1, TABLE_HEADER)).unwrap();
    writeln!(output, "{TABLE_RULE}").unwrap();
    for block in &analysis.blocks {
        let meaning = block_meaning(block, &analysis.bytes, MEANING_WIDTH);
        let padded_meaning = format!("{meaning:<MEANING_WIDTH$}");
        let colored_meaning = match meaning.as_str() {
            value if value.starts_with("root") => paint(color, 35, padded_meaning),
            value if value.starts_with("probable string") => paint(color, 33, padded_meaning),
            _ => paint(color, 34, padded_meaning),
        };
        writeln!(
            output,
            "#{:03}   0x{:08x}  0x{:08x}  {}  {:>8}  {:>8}  {:>8} {:>3} {:>3}",
            block.id,
            block.header_offset,
            block.payload_offset,
            colored_meaning,
            block.count,
            block.stride,
            block.payload_size(),
            incoming_count(analysis, block.id),
            outgoing_count(analysis, block.id),
        )
        .unwrap();
    }

    writeln!(output, "\n{}", paint(color, 36, "Diagnostics:")).unwrap();
    writeln!(
        output,
        "  file: {} bytes, {}",
        analysis.bytes.len(),
        analysis.endian
    )
    .unwrap();
    writeln!(
        output,
        "  reachability: {reachable_count}/{} blocks from root",
        analysis.blocks.len()
    )
    .unwrap();
    writeln!(
        output,
        "  offsets: {strong_edges} strong, {candidate_edges} lower-confidence"
    )
    .unwrap();
    if analysis.warnings.is_empty() {
        writeln!(output, "  warnings: none").unwrap();
    } else {
        writeln!(output, "  warnings:").unwrap();
        for warning in &analysis.warnings {
            writeln!(output, "    - {warning}").unwrap();
        }
    }

    writeln!(output, "\n{}", paint(color, 32, "Inferred offset edges:")).unwrap();
    if strong_edges == 0 {
        writeln!(output, "  (none)").unwrap();
    } else {
        for edge in analysis
            .edges
            .iter()
            .filter(|edge| edge.confidence == EdgeConfidence::Strong)
        {
            write_edge(&mut output, edge);
        }
    }

    if candidate_edges != 0 {
        writeln!(
            output,
            "\n{}",
            paint(
                color,
                33,
                "Backward/self boundary matches (lower confidence)"
            )
        )
        .unwrap();
        for edge in analysis
            .edges
            .iter()
            .filter(|edge| edge.confidence == EdgeConfidence::Candidate)
        {
            write_edge(&mut output, edge);
        }
    }
    output
}

pub(crate) fn block_meaning(block: &Block, bytes: &[u8], width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let meaning = if block.id == 0 {
        "root record".to_owned()
    } else if block.text_preview(bytes).is_some() {
        format!("probable string \"{}\"", display_text(block, bytes))
    } else if block.stride == 2 {
        "16-bit scalar-shaped array".to_owned()
    } else if block.stride == 1 {
        "byte array".to_owned()
    } else if block.count == 1 {
        "opaque record".to_owned()
    } else {
        "record array".to_owned()
    };
    truncate(&meaning, width)
}

fn display_text(block: &Block, bytes: &[u8]) -> String {
    let text = std::str::from_utf8(&bytes[block.payload_offset..block.payload_end]).unwrap();
    let mut display = String::new();
    for character in text.chars() {
        match character {
            '\n' => display.push_str("\\n"),
            '\r' => display.push_str("\\r"),
            '\t' => display.push_str("\\t"),
            '"' => display.push_str("\\\""),
            value if value.is_control() => display.extend(value.escape_default()),
            value => display.push(value),
        }
    }
    display
}

fn truncate(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_owned();
    }
    value.chars().take(width - 1).chain(['…']).collect()
}

pub(crate) fn incoming_count(analysis: &Analysis, block_id: usize) -> usize {
    analysis
        .edges
        .iter()
        .filter(|edge| edge.target_block == block_id)
        .count()
}

pub(crate) fn outgoing_count(analysis: &Analysis, block_id: usize) -> usize {
    analysis
        .edges
        .iter()
        .filter(|edge| edge.source_block == block_id)
        .count()
}

fn write_edge(output: &mut String, edge: &Edge) {
    writeln!(
        output,
        "  B{} record {} +0x{:x} @0x{:08x} -> B{}",
        edge.source_block,
        edge.record_index,
        edge.field_offset,
        edge.file_offset,
        edge.target_block
    )
    .unwrap();
}

/// Render the high-confidence offset graph as an ASCII tree-like view.
pub fn graph(analysis: &Analysis) -> String {
    let mut output = String::from("RDM inferred offset graph\n");
    let mut visited = vec![false; analysis.blocks.len()];
    graph_block(analysis, 0, "", &mut visited, &mut output);

    let omitted: Vec<_> = visited
        .iter()
        .enumerate()
        .filter_map(|(id, &seen)| (!seen).then_some(id))
        .collect();
    if !omitted.is_empty() {
        writeln!(output, "\nNot reached by strong edges: {omitted:?}").unwrap();
    }
    let candidates = analysis
        .edges
        .iter()
        .filter(|edge| edge.confidence == EdgeConfidence::Candidate)
        .count();
    if candidates != 0 {
        writeln!(
            output,
            "Lower-confidence backward/self matches omitted: {candidates}"
        )
        .unwrap();
    }
    output
}

fn graph_block(
    analysis: &Analysis,
    block_id: usize,
    prefix: &str,
    visited: &mut [bool],
    output: &mut String,
) {
    let block = &analysis.blocks[block_id];
    let revisited = std::mem::replace(&mut visited[block_id], true);
    writeln!(
        output,
        "{prefix}B{block_id:03} {}  count {}  stride {}{}",
        analysis.structural_label(block.id),
        block.count,
        block.stride,
        if revisited {
            "  [shared/revisited]"
        } else {
            ""
        }
    )
    .unwrap();
    if revisited {
        return;
    }

    let outgoing: Vec<_> = analysis
        .edges
        .iter()
        .filter(|edge| edge.source_block == block_id && edge.confidence == EdgeConfidence::Strong)
        .collect();
    for (index, edge) in outgoing.iter().enumerate() {
        let last = index + 1 == outgoing.len();
        writeln!(
            output,
            "{prefix}{} field r{}+0x{:x} @0x{:x}",
            if last { "`--" } else { "|--" },
            edge.record_index,
            edge.field_offset,
            edge.file_offset
        )
        .unwrap();
        let child_prefix = format!("{prefix}{}   ", if last { " " } else { "|" });
        graph_block(analysis, edge.target_block, &child_prefix, visited, output);
    }
}

/// Render all blocks and inferred fields in Graphviz DOT format.
pub fn dot(analysis: &Analysis) -> String {
    let mut output = String::from("digraph rdm {\n  rankdir=LR;\n  node [shape=box];\n");
    for block in &analysis.blocks {
        let label = analysis.structural_label(block.id).replace('"', "\\\"");
        writeln!(
            output,
            "  B{} [label=\"B{}: {}\\ncount {}  stride {}\\npayload 0x{:x}\"];",
            block.id, block.id, label, block.count, block.stride, block.payload_offset
        )
        .unwrap();
    }
    for edge in &analysis.edges {
        let style = if edge.confidence == EdgeConfidence::Strong {
            "color=\"#2ca02c\""
        } else {
            "color=\"#999999\", style=dashed"
        };
        writeln!(
            output,
            "  B{} -> B{} [label=\"r{}+0x{:x}\", {}];",
            edge.source_block, edge.target_block, edge.record_index, edge.field_offset, style
        )
        .unwrap();
    }
    output.push_str("}\n");
    output
}

#[cfg(test)]
mod tests {
    use crate::Analysis;

    #[test]
    fn summary_uses_compact_block_table_and_counts_all_boundary_matches() {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../rdm4lib/rdm/fishery_others_lod2.rdm");
        let analysis = Analysis::parse(std::fs::read(fixture).unwrap()).unwrap();
        let output = super::summary(&analysis, false);

        assert!(output.starts_with(super::TABLE_HEADER));
        assert!(output.contains(
            "#000   0x00000014  0x0000001c  root record                            1        48        48   1   3"
        ));
        assert!(output.contains(
            "#002   0x0000009c  0x000000a4  probable string \"G:\\graphic_b…       100         1       100   1   0"
        ));
        assert!(output.contains(
            "#010   0x0000024b  0x00000253  record array                           2        28        56   1   1"
        ));
        assert!(output.contains(
            "#012   0x0001371b  0x00013723  16-bit scalar-shaped array          7473         2     14946   1   0"
        ));
        assert_eq!(output.matches("Diagnostics:").count(), 1);
        assert_eq!(output.matches(super::TABLE_HEADER).count(), 1);
        assert!(output.contains("Inferred offset edges:\n"));
        assert!(output.contains("Backward/self boundary matches (lower confidence)\n"));
    }

    #[test]
    fn dot_output_is_a_complete_graph() {
        let bytes = b"RDM\x01\x14\0\0\0\0\0\0\0\x04\0\0\0\x1c\0\0\0\x01\0\0\0\x08\0\0\0\x2c\0\0\0\0\0\0\0\x05\0\0\0\x01\0\0\0hello".to_vec();
        let analysis = Analysis::parse(bytes).unwrap();
        let output = super::dot(&analysis);
        assert!(output.starts_with("digraph rdm {"));
        assert!(output.contains("B0 -> B1"));
        assert!(output.ends_with("}\n"));
    }
}
