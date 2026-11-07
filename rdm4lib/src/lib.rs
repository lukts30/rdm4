//! Typed RDM codecs and scene conversion for the rdm4 CLIs.

#[macro_use]
extern crate log;

mod gltf_buffers;
pub mod gltf_export;
pub mod gltf_reader;
pub mod rdm;
pub mod rdm_anim;
pub mod rdm_material;
pub mod scene;
pub mod vertex;
pub mod vertex_transform;

#[cfg(test)]
mod tests_intern {
    use crate::{rdm::*, vertex::StorageType};
    use std::io::Cursor;

    fn read(bytes: &[u8]) -> RdmDocument {
        RdmReader::new(Cursor::new(bytes))
            .unwrap()
            .read_root::<RdmDocument>()
            .unwrap()
            .value
    }

    #[test]
    fn reads_u32_indices_without_truncation() {
        let source = read(include_bytes!(
            "../rdm/destruction_stone_building_04_decal_detail01_lod0.rdm"
        ));
        let model = &source.models.as_ref().unwrap()[0];
        assert_eq!(model.indices.values.len() / 3, 37_240);
        assert_eq!(model.indices.values.iter().max(), Some(&74_479));
        assert_eq!(model.index_format.storage_type, StorageType::U32);
        let output = RdmWriter::new(Cursor::new(Vec::new()), RdmEndian::Little)
            .write_root(&source)
            .unwrap()
            .into_inner();
        assert_eq!(read(&output), source);
    }

    #[test]
    fn rejects_unknown_float_and_mismatched_index_types() {
        let bytes = include_bytes!("../rdm/fishery_others_lod2.rdm");
        let word =
            |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let meta = word(word(16) + 4);
        let index_format = word(meta + 8);
        for (code, case) in [
            (99u32, "unknown storage code"),
            (5, "U32 descriptor with a U16 index buffer"),
            (6, "F16 indices"),
            (7, "F32 indices"),
        ] {
            let mut invalid = bytes.to_vec();
            invalid[index_format..index_format + 4].copy_from_slice(&code.to_le_bytes());
            assert!(
                RdmReader::new(std::io::Cursor::new(invalid))
                    .unwrap()
                    .read_root::<RdmDocument>()
                    .is_err(),
                "accepted {case}"
            );
        }
    }

    #[test]
    fn decodes_be_vertices_and_both_index_widths_like_le() {
        let bytes = include_bytes!("../rdm/fishery_others_lod2.rdm");
        let expected = read(bytes);
        for storage_type in [StorageType::U16, StorageType::U32] {
            let mut document = RdmReader::new(std::io::Cursor::new(bytes))
                .unwrap()
                .read_root::<RdmDocument>()
                .unwrap()
                .value;
            document.models.as_mut().unwrap()[0]
                .index_format
                .storage_type = storage_type;
            let output = RdmWriter::new(std::io::Cursor::new(Vec::new()), RdmEndian::Big)
                .write_root(&document)
                .unwrap()
                .into_inner();
            let actual = read(&output);
            let actual = &actual.models.as_ref().unwrap()[0];
            let expected = &expected.models.as_ref().unwrap()[0];
            assert_eq!(actual.vertices, expected.vertices);
            assert_eq!(actual.indices, expected.indices, "{storage_type:?} indices");
        }
    }

    #[test]
    fn reads_position_only_mesh_from_memory() {
        // for Miri test include bytes since there is no i/o.
        let bytes = include_bytes!("../rdm/fishery_others_cutout_lod0.rdm");
        let document = read(bytes);
        let model = &document.models.as_ref().unwrap()[0];
        assert_eq!(model.vertices.count, 32);
        assert_eq!(model.vertex_format.to_string(), "P4h");
        assert_eq!(model.indices.values.len(), 78);
    }
}
