//! Big-endian decoding, checked against values read directly from the file.

use std::{fs, path::PathBuf};

use rdm_core::{AttributeValues, Document, Endian, Indices, VertexSemantic};

fn fixture(name: &str) -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../rdm4lib/rdm")
            .join(name),
    )
    .unwrap()
}

fn be_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap())
}

#[test]
fn decodes_big_endian_model() {
    let input = fixture("n_nature_small_stone_02_lod0.rdm");
    let document = Document::from_bytes(&input).unwrap();
    assert_eq!(document.endian, Endian::Big);
    assert_eq!(document.to_bytes().unwrap(), input);

    // Follow Root.models -> Model.vertices / Model.indices by hand.
    let model_at = be_u32(&input, 0x1c + 4) as usize;
    let vertices_at = be_u32(&input, model_at + 12) as usize;
    let indices_at = be_u32(&input, model_at + 16) as usize;
    let vertex_count = be_u32(&input, vertices_at - 8) as usize;
    let vertex_stride = be_u32(&input, vertices_at - 4) as usize;

    let model = &document.root.models.as_ref().unwrap()[0];
    assert_eq!(model.vertices.count, vertex_count);
    let bounds_min: [f32; 3] =
        std::array::from_fn(|i| f32::from_bits(be_u32(&input, model_at + 28 + 4 * i)));
    assert_eq!(model.bounds_min, bounds_min);

    let Indices::U16(indices) = &model.indices else {
        panic!("expected U16 indices");
    };
    let first_index = u16::from_be_bytes(input[indices_at..indices_at + 2].try_into().unwrap());
    assert_eq!(indices[0], first_index);

    let attributes = &model.vertex_format.attributes;
    assert_eq!(attributes[0].semantic, VertexSemantic::Position);
    let AttributeValues::F16(positions) = &model.vertices.attributes[0] else {
        panic!("expected F16 positions");
    };
    let second_vertex_x = u16::from_be_bytes(
        input[vertices_at + vertex_stride..vertices_at + vertex_stride + 2]
            .try_into()
            .unwrap(),
    );
    let components = attributes[0].components as usize;
    assert_eq!(positions[components].to_bits(), second_vertex_x);

    // Packed normals are one BE word; rdm4lib decodes the first normal
    // as (0.90588236, 0.42745098, 0.03529412), i.e. bytes 243, 182, 132.
    let normal = attributes
        .iter()
        .position(|a| a.semantic == VertexSemantic::Normal)
        .unwrap();
    let AttributeValues::U32(normals) = &model.vertices.attributes[normal] else {
        panic!("expected packed normals");
    };
    assert_eq!(normals[0].to_le_bytes()[..3], [243, 182, 132]);
}

/// Writing decoded records in the other byte order and reading them back
/// must reproduce every decoded value. Unknown bytes are not byte-swapped,
/// so this only checks the typed codecs.
#[test]
fn decoded_values_survive_both_byte_orders() {
    for name in [
        "benchmark/03-impostor.rdm",
        "benchmark/09-hop_worker_stand_01.rdm",
        "benchmark/15-mining_08_slot_01_lod0.rdm",
        "benchmark/21-3rd_party_08.rdm",
        "n_nature_small_stone_02_lod0.rdm",
    ] {
        let input = fixture(name);
        let original = Document::from_bytes(&input).unwrap();

        let mut swapped = original.clone();
        swapped.endian = match original.endian {
            Endian::Little => Endian::Big,
            Endian::Big => Endian::Little,
        };
        let swapped_bytes = swapped.to_bytes().unwrap();
        assert_ne!(swapped_bytes, input, "{name}");

        let mut decoded = Document::from_bytes(&swapped_bytes).unwrap();
        assert_eq!(decoded.root, original.root, "{name}");

        decoded.endian = original.endian;
        assert_eq!(decoded.to_bytes().unwrap(), input, "{name}");
    }
}
