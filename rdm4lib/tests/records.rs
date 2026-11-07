use std::{
    fs,
    io::{BufReader, Cursor},
    path::PathBuf,
};

use rdm4lib::rdm::{
    RdmAttributeValues, RdmEndian, RdmHeader, RdmIndexFormat, RdmIndices, RdmMeshGroup, RdmReader,
    RdmVertexFormat, RdmVertices, RdmWriteField, RdmWriteRecord, RdmWriter,
};
use rdm4lib::vertex::{StorageType, VertexIdentifier, VertexInterpretation, VertexSemantic};
use serde_json::Value;

fn sample(endian: RdmEndian, stride: u32) -> Vec<u8> {
    let mut bytes = RdmHeader { endian }.encode().to_vec();
    for value in [2, stride] {
        bytes.extend_from_slice(&match endian {
            RdmEndian::Little => value.to_le_bytes(),
            RdmEndian::Big => value.to_be_bytes(),
        });
    }
    for values in [[3u32, 6, 1], [9, 12, 2]] {
        for value in values {
            bytes.extend_from_slice(&match endian {
                RdmEndian::Little => value.to_le_bytes(),
                RdmEndian::Big => value.to_be_bytes(),
            });
        }
        bytes.extend(std::iter::repeat_n(0xa5, stride as usize - 12));
    }
    bytes
}

#[test]
fn indexed_records_are_owned_and_preserve_all_tail_bytes_in_both_orders() {
    for endian in [RdmEndian::Little, RdmEndian::Big] {
        let decoded = {
            let bytes = sample(endian, 28);
            let mut reader = RdmReader::new(Cursor::new(&bytes)).unwrap();
            assert_eq!(reader.endian(), endian);
            reader.read_block::<RdmMeshGroup>(28).unwrap()
        };
        assert_eq!(
            decoded.records,
            [
                RdmMeshGroup {
                    first_index: 3,
                    index_count: 6,
                    material: 1
                },
                RdmMeshGroup {
                    first_index: 9,
                    index_count: 12,
                    material: 2
                },
            ]
        );
        assert_eq!(decoded.source.payload, 28..84);
        assert_eq!(decoded.tails, [0xa5; 32]);
        let expected = sample(endian, 28);
        let mut output = expected.clone();
        for at in [28, 56] {
            output[at..at + 12].fill(0);
        }
        let mut writer = RdmWriter::new(Cursor::new(output), endian);
        for (i, record) in decoded.records.iter().enumerate() {
            record.write(&mut writer, 28 + i * 28).unwrap();
        }
        assert_eq!(writer.into_inner().into_inner(), expected);
    }
}

#[test]
fn rejects_non_boundary_targets_and_unsupported_strides() {
    let mut reader = RdmReader::new(Cursor::new(sample(RdmEndian::Little, 28))).unwrap();
    for target in [0, 20, 29, 84, u32::MAX] {
        let error = reader.read_block::<RdmMeshGroup>(target).unwrap_err();
        assert_eq!(error.offset(), target as usize);
        assert_eq!(error.context(), "RdmMeshGroup");
        assert_eq!(error.expected(), "indexed block payload boundary");
    }
    let mut reader = RdmReader::new(Cursor::new(sample(RdmEndian::Little, 24))).unwrap();
    let error = reader.read_block::<RdmMeshGroup>(28).unwrap_err();
    assert_eq!(error.offset(), 24);
    assert_eq!(error.actual(), "24");
    assert_eq!(error.expected(), "stride [28] covering 12 known bytes");
}

#[test]
fn typed_reader_requires_supported_preamble_and_complete_framing() {
    let mut bytes = sample(RdmEndian::Little, 28);
    bytes[12] = 7;
    let error = RdmReader::new(Cursor::new(&bytes)).unwrap_err();
    assert_eq!(error.offset(), 12);
    bytes[12] = 4;
    bytes.pop();
    let error = RdmReader::new(Cursor::new(bytes)).unwrap_err();
    assert_eq!(error.context(), "block payload");
}

#[test]
fn descriptors_report_unknown_codes_and_preserve_interpretation_in_both_orders() {
    for endian in [RdmEndian::Little, RdmEndian::Big] {
        for (semantic, storage, error) in [
            (0, 6, None),
            (255, 3, None),
            (99, 7, Some((28, "VertexIdentifier.semantic"))),
            (0, 99, Some((32, "VertexIdentifier.storage_type"))),
        ] {
            let mut bytes = RdmHeader { endian }.encode().to_vec();
            for value in [1u32, 16, semantic, storage, u32::MAX, 4] {
                bytes.extend_from_slice(&match endian {
                    RdmEndian::Little => value.to_le_bytes(),
                    RdmEndian::Big => value.to_be_bytes(),
                });
            }
            let result = RdmReader::new(Cursor::new(bytes))
                .unwrap()
                .read_block::<VertexIdentifier>(28);
            if let Some((offset, context)) = error {
                let error = result.unwrap_err();
                assert_eq!(error.offset(), offset);
                assert_eq!(error.context(), context);
                assert_eq!(error.actual(), "99");
            } else {
                let descriptor = result.unwrap().records.pop().unwrap();
                assert_eq!(descriptor.storage_type as u32, storage);
                assert_eq!(descriptor.semantic as u32, semantic);
                assert_eq!(descriptor.interpretation.0, u32::MAX);
                assert_eq!(descriptor.storage_count, 4);
            }
        }
    }
}

fn word(bytes: &[u8], at: usize) -> usize {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize
}

#[test]
fn vertex_format_selector_is_a_full_word_in_both_orders() {
    let format = RdmVertexFormat {
        selector: 0x12345678,
        attributes: vec![VertexIdentifier {
            semantic: VertexSemantic::Position,
            storage_type: StorageType::F32,
            interpretation: VertexInterpretation(0),
            storage_count: 3,
        }],
    };
    for endian in [RdmEndian::Little, RdmEndian::Big] {
        let bytes = RdmWriter::new(Cursor::new(Vec::new()), endian)
            .write_root(&format)
            .unwrap()
            .into_inner();
        let expected = match endian {
            RdmEndian::Little => [0x78, 0x56, 0x34, 0x12],
            RdmEndian::Big => [0x12, 0x34, 0x56, 0x78],
        };
        assert_eq!(bytes[32..36], expected);
        let decoded = RdmReader::new(Cursor::new(&bytes))
            .unwrap()
            .read_root::<RdmVertexFormat>()
            .unwrap();
        assert_eq!(decoded.value, format);
        assert_eq!(decoded.layout.blocks[0].known_size, 8);
        let output = RdmWriter::new(Cursor::new(Vec::new()), endian)
            .write_preserving(&decoded.value, decoded.layout)
            .unwrap()
            .into_inner();
        assert_eq!(output, bytes);
    }
}

#[test]
fn reads_corpus_material_ranges_from_buffered_files() {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rdm/benchmark");
    let manifest: Vec<Value> =
        serde_json::from_str(include_str!("../rdm/benchmark/manifest.json")).unwrap();
    let expected: Vec<Value> =
        serde_json::from_str(include_str!("../rdm/benchmark/expectations.json")).unwrap();
    let mut meshes = 0;
    for (fixture, expected) in manifest.iter().zip(expected) {
        if expected["model_count"] == 0 {
            continue;
        }
        let path = base.join(fixture["path"].as_str().unwrap());
        let bytes = fs::read(&path).unwrap();
        let first_model = word(&bytes, 0x20);
        let groups = word(&bytes, first_model + 0x14);
        let mut reader = RdmReader::new(BufReader::new(fs::File::open(&path).unwrap())).unwrap();
        let decoded = reader.read_block::<RdmMeshGroup>(groups as u32).unwrap();
        let values: Vec<_> = decoded
            .records
            .iter()
            .map(|group| [group.first_index, group.index_count, group.material])
            .collect();
        assert_eq!(
            serde_json::json!(values),
            expected["first_model"]["groups"],
            "{path:?}"
        );
        let tails: Vec<_> = bytes[decoded.source.payload.clone()]
            .as_chunks::<28>()
            .0
            .iter()
            .flat_map(|record| record[12..].iter().copied())
            .collect();
        assert_eq!(decoded.tails, tails, "{path:?}");
        meshes += 1;
    }
    assert_eq!(meshes, 18);
}

#[test]
fn buffer_writes_reject_invalid_columns_and_indices_before_appending() {
    let format = |components| RdmVertexFormat {
        selector: 0,
        attributes: vec![VertexIdentifier {
            semantic: VertexSemantic::Position,
            storage_type: StorageType::F32,
            interpretation: VertexInterpretation(0),
            storage_count: components,
        }],
    };
    let mut writer = RdmWriter::new(Cursor::new(vec![0xa5; 36]), RdmEndian::Little);
    for attributes in [
        vec![],
        vec![RdmAttributeValues::U16(vec![1, 2])],
        vec![RdmAttributeValues::F32(vec![1.0])],
    ] {
        let vertices = RdmVertices {
            count: 2,
            attributes,
        };
        let error = vertices
            .write_field(&mut writer, &mut [0; 4], 28, "vertices", &format(1))
            .unwrap_err();
        assert_eq!((error.context(), error.offset()), ("vertices", 28));
    }
    let vertices = RdmVertices {
        count: 2,
        attributes: vec![RdmAttributeValues::F32(vec![1.0, 2.0])],
    };
    for format in [format(0), format(u32::MAX)] {
        let error = vertices
            .write_field(&mut writer, &mut [0; 4], 28, "vertices", &format)
            .unwrap_err();
        assert_eq!(error.expected(), "nonzero vertex stride fitting u32");
    }
    for (storage_type, count, value, expected) in [
        (StorageType::F32, 2, 0, "index storage U16 or U32"),
        (StorageType::U32, 2, 2, "index below vertex count 2"),
        (StorageType::U16, 65_537, 65_536, "index fitting U16"),
    ] {
        let format = RdmIndexFormat { storage_type };
        let vertices = RdmVertices {
            count,
            attributes: Vec::new(),
        };
        let error = RdmIndices {
            values: vec![value],
        }
        .write_field(
            &mut writer,
            &mut [0; 4],
            32,
            "indices",
            (&format, &vertices),
        )
        .unwrap_err();
        assert_eq!((error.context(), error.offset()), ("indices", 32));
        assert_eq!(error.expected(), expected);
    }
    assert_eq!(writer.into_inner().into_inner(), [0xa5; 36]);
}
