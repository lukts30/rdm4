use crate::{rdm_material::RdMaterial, vertex::*, MeshInstance, RdJoint, RdModell};
use gltf::{json, json::validation::Checked::Valid, mesh::Semantic};
use std::{
    borrow::Cow,
    collections::HashMap,
    convert::TryInto,
    env,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::PathBuf,
    str::FromStr,
};

use bytes::{BufMut, Bytes, BytesMut};
use nalgebra::*;

#[derive(Debug)]
#[repr(C)]
struct Vertex {
    position: [f32; 3],
}

#[derive(Debug, Eq, Hash, PartialEq)]
#[allow(dead_code)]
pub enum JointOption {
    ResolveParentNode,
    ResolveAllRoot,
}

struct RdGltfBuilder {
    buffers: Vec<json::Buffer>,
    buffer_views: Vec<json::buffer::View>,
    accessors: Vec<json::Accessor>,
    nodes: Vec<json::Node>,
    attr_map: HashMap<json::validation::Checked<Semantic>, json::Index<json::Accessor>>,
    idx: Option<Vec<u32>>,

    rdm: RdModell,
    obj: RdGltf, // private
    skin: Option<json::Skin>,
    anim_node: Option<json::Animation>,
    material_idx: Option<Vec<u32>>,
    material_vec: Vec<json::Material>,
    texture_vec: Vec<json::Texture>,
    image_vec: Vec<json::Image>,
    sampler_vec: Vec<json::texture::Sampler>,
}

impl RdGltfBuilder {
    fn new(rdm: RdModell) -> Self {
        RdGltfBuilder {
            buffers: Vec::new(),
            buffer_views: Vec::new(),
            accessors: Vec::new(),
            nodes: Vec::new(),
            attr_map: HashMap::new(),
            rdm,
            obj: RdGltf::new(),
            skin: None,
            idx: None,
            anim_node: None,
            material_idx: None,
            material_vec: vec![],
            texture_vec: vec![],
            image_vec: vec![],
            sampler_vec: vec![],
        }
    }

    pub fn put_rdm_anim(&mut self) {
        // TODO: must not circumvent PushBufferResult
        let buffv_idx = self.buffers.len() as u32;
        let mut bv_idx = self.buffer_views.len() as u32;
        let mut acc_idx = self.accessors.len() as u32;

        let anim = self.rdm.anim.clone().unwrap();
        let anim_vec = anim.anim_vec.clone();

        let mut size: usize = 0;
        for janim in &anim_vec {
            size += janim.len as usize;
        }

        let rot_size = size * 16;
        let trans_size = size * 12;
        let t_size = size * 4;

        let mut rot_anim_buf = BytesMut::with_capacity(rot_size);
        let mut trans_anim_buf = BytesMut::with_capacity(trans_size);
        let mut t_anim_buf = BytesMut::with_capacity(t_size);

        let mut buffer_v_vec = Vec::new();

        let mut acc_vec = Vec::new();
        let time_1000_f32_max = (anim.time_max as f32) / 1000.0;

        let mut rot_sampler_chanel = 0;
        let mut trans_sampler_chanel = 1;

        let mut sampler_vec = Vec::new();
        let mut chanel_vec = Vec::new();

        let p = self.rdm.joints.clone().unwrap();
        let mut modell_nodes = HashMap::new();

        for (i, joint) in p.iter().enumerate() {
            modell_nodes.insert(joint.name.clone(), i);
        }
        for (_, janim) in anim_vec.iter().enumerate() {
            let target_node_idx = match modell_nodes.get(&janim.name) {
                Some(idx) => *idx as u32,
                None => {
                    warn!(
                        "Could not find animation target {:?} in base model {:?}",
                        &janim.name, p
                    );
                    //TODO: proper fix for unused animation targets.
                    for _ in 0..janim.frames.len() {
                        rot_anim_buf.put_u32_le(0xDEAD_BEEF);
                        rot_anim_buf.put_u32_le(0xDEAD_BEEF);
                        rot_anim_buf.put_u32_le(0xDEAD_BEEF);
                        rot_anim_buf.put_u32_le(0xDEAD_BEEF);

                        trans_anim_buf.put_u32_le(0xDEAD_BEEF);
                        trans_anim_buf.put_u32_le(0xDEAD_BEEF);
                        trans_anim_buf.put_u32_le(0xDEAD_BEEF);

                        t_anim_buf.put_u32_le(0xDEAD_BEEF);
                    }
                    continue;
                }
            };

            let count = janim.len as usize;

            let rot_start = rot_anim_buf.len();
            let trans_start = trans_anim_buf.len();
            let t_start = t_anim_buf.len();

            let mut time_real_f32_max = 0.0f32;

            for f in &janim.frames {
                rot_anim_buf.put_f32_le(f.rotation[0]);
                rot_anim_buf.put_f32_le(f.rotation[1]);
                rot_anim_buf.put_f32_le(f.rotation[2]);
                rot_anim_buf.put_f32_le(-f.rotation[3]);

                trans_anim_buf.put_f32_le(f.translation[0]);
                trans_anim_buf.put_f32_le(f.translation[1]);
                trans_anim_buf.put_f32_le(f.translation[2]);

                t_anim_buf.put_f32_le(f.time);
                time_real_f32_max = time_real_f32_max.max(f.time);
            }
            debug!("time_real_f32_max: {}", time_real_f32_max);
            debug!("time_1000_f32_max: {}", time_1000_f32_max);

            let rot_end = rot_anim_buf.len();
            trace!("{}", rot_start);
            trace!("{}", rot_end);

            let trans_end = trans_anim_buf.len();
            let t_end = t_anim_buf.len();

            let rot_real_len = rot_end - rot_start;
            let trans_real_len = trans_end - trans_start;

            let t_real_len = t_end - t_start;

            let rot_buffer_view = json::buffer::View {
                buffer: json::Index::new(buffv_idx),
                byte_length: rot_real_len as u32,
                byte_offset: Some(rot_start as u32),
                byte_stride: None,
                extensions: Default::default(),
                extras: Default::default(),
                name: None,
                target: None,
            };

            let rot_accessor = json::Accessor {
                buffer_view: Some(json::Index::new(bv_idx)),
                byte_offset: 0,
                count: count as u32,
                component_type: Valid(json::accessor::GenericComponentType(
                    json::accessor::ComponentType::F32,
                )),
                extensions: Default::default(),
                extras: Default::default(),
                type_: Valid(json::accessor::Type::Vec4),
                min: None,
                max: None,
                name: None,
                normalized: false,
                sparse: None,
            };
            let rot_sampler_idx = acc_idx;
            acc_vec.push(rot_accessor);
            acc_idx += 1;
            bv_idx += 1;

            let trans_buffer_view = json::buffer::View {
                buffer: json::Index::new(buffv_idx),
                byte_length: trans_real_len as u32,
                byte_offset: Some((rot_size + trans_start) as u32),
                byte_stride: None,
                extensions: Default::default(),
                extras: Default::default(),
                name: None,
                target: None,
            };

            let trans_accessor = json::Accessor {
                buffer_view: Some(json::Index::new(bv_idx)),
                byte_offset: 0,
                count: count as u32,
                component_type: Valid(json::accessor::GenericComponentType(
                    json::accessor::ComponentType::F32,
                )),
                extensions: Default::default(),
                extras: Default::default(),
                type_: Valid(json::accessor::Type::Vec3),
                min: None,
                max: None,
                name: None,
                normalized: false,
                sparse: None,
            };
            let trans_sampler_idx = acc_idx;
            acc_vec.push(trans_accessor);
            acc_idx += 1;
            bv_idx += 1;

            let time_buffer_view = json::buffer::View {
                buffer: json::Index::new(buffv_idx),
                byte_length: t_real_len as u32,
                byte_offset: Some((rot_size + trans_size + t_start) as u32),
                byte_stride: None,
                extensions: Default::default(),
                extras: Default::default(),
                name: None,
                target: None,
            };

            let time_accessor = json::Accessor {
                buffer_view: Some(json::Index::new(bv_idx)),
                byte_offset: 0,
                count: count as u32,
                component_type: Valid(json::accessor::GenericComponentType(
                    json::accessor::ComponentType::F32,
                )),
                extensions: Default::default(),
                extras: Default::default(),
                type_: Valid(json::accessor::Type::Scalar),
                min: Some(json::Value::from(vec![0.0])),
                max: Some(json::Value::from(vec![time_real_f32_max])),
                name: None,
                normalized: false,
                sparse: None,
            };
            let time_sampler_idx = acc_idx;
            acc_vec.push(time_accessor);
            acc_idx += 1;
            bv_idx += 1;

            buffer_v_vec.push(rot_buffer_view);
            buffer_v_vec.push(trans_buffer_view);
            buffer_v_vec.push(time_buffer_view);

            let rot_sampler = json::animation::Sampler {
                input: json::Index::new(time_sampler_idx),
                interpolation: Valid(json::animation::Interpolation::Linear),
                output: json::Index::new(rot_sampler_idx),
                extensions: None,
                extras: None,
            };

            let trans_sampler = json::animation::Sampler {
                input: json::Index::new(time_sampler_idx),
                interpolation: Valid(json::animation::Interpolation::Linear),
                output: json::Index::new(trans_sampler_idx),
                extensions: None,
                extras: None,
            };

            let rot_chanel = json::animation::Channel {
                sampler: json::Index::new(rot_sampler_chanel),
                target: json::animation::Target {
                    node: json::Index::new(target_node_idx as u32),
                    path: Valid(json::animation::Property::Rotation),
                    extensions: None,
                    extras: None,
                },
                extensions: None,
                extras: None,
            };

            let trans_chanel = json::animation::Channel {
                sampler: json::Index::new(trans_sampler_chanel),
                target: json::animation::Target {
                    node: json::Index::new(target_node_idx as u32),
                    path: Valid(json::animation::Property::Translation),
                    extensions: None,
                    extras: None,
                },
                extensions: None,
                extras: None,
            };

            sampler_vec.push(rot_sampler);
            sampler_vec.push(trans_sampler);

            chanel_vec.push(rot_chanel);
            chanel_vec.push(trans_chanel);

            rot_sampler_chanel += 2;
            trans_sampler_chanel += 2;
        }

        let anim_node = json::animation::Animation {
            name: Some(anim.name),
            samplers: sampler_vec,
            channels: chanel_vec,
            extensions: None,
            extras: None,
        };

        debug!("{:#?}", anim_node);

        let mut b1 = rot_anim_buf.to_vec();
        let mut b2 = trans_anim_buf.to_vec();
        let mut b3 = t_anim_buf.to_vec();

        b1.append(&mut b2);
        b1.append(&mut b3);
        let buffer_result = self.obj.push_buffer(BufferContainer::U8(b1));
        assert_eq!(buffv_idx, buffer_result.idx);

        let anim_buffer = json::Buffer {
            byte_length: (rot_anim_buf.len() + trans_anim_buf.len() + t_anim_buf.len()) as u32,
            extensions: Default::default(),
            extras: Default::default(),
            name: None,
            uri: Some(buffer_result.file_name),
        };

        self.buffers.push(anim_buffer);

        self.buffer_views.append(&mut buffer_v_vec);

        self.accessors.append(&mut acc_vec);

        self.anim_node = Some(anim_node);
    }

    fn put_joint_weight(&mut self, normalise: bool) {
        if normalise {
            self.rdm.vertex.set_weight_sum();
        }
        let n = self
            .rdm
            .vertex
            .find_component_offsets(UniqueIdentifier::I4b)
            .count();
        let mut ibuffers = Vec::with_capacity(n);
        let mut wbuffers = Vec::with_capacity(n);
        for i in 0..n {
            if let Some(iter) = self.rdm.vertex.iter::<I4b, I4b>(i) {
                let mut weight_buf =
                    BytesMut::with_capacity((4 + 4 * 4) * self.rdm.vertex.len() as usize);
                let mut joint_buf = BytesMut::with_capacity(4 * self.rdm.vertex.len() as usize);

                let (mut a, mut b);
                let w4b_iter: &mut dyn Iterator<Item = W4b> =
                    match self.rdm.vertex.iter::<W4b, W4b>(i) {
                        Some(w) => {
                            a = w;
                            &mut a
                        }
                        None => {
                            b = self.rdm.vertex.w4b_default_iter();
                            &mut b
                        }
                    };

                // TODO: fix weight_sum unwrap (normalise == false)
                for ((vjoint, vweight), sum) in iter
                    .zip(w4b_iter)
                    .zip(self.rdm.vertex.weight_sum.as_ref().unwrap())
                {
                    if normalise {
                        // ACCESSOR_JOINTS_USED_ZERO_WEIGHT
                        for (w, j) in vweight.data.iter().zip(vjoint.data.iter()) {
                            if *w == 0 {
                                joint_buf.put_u8(0);
                            } else {
                                joint_buf.put_u8(*j);
                            }
                            // ACCESSOR_WEIGHTS_NON_NORMALIZED
                            let sum_float: f32 = 255.0f32 / *sum as f32;
                            weight_buf.put_f32_le(*w as f32 / 255.0 * sum_float);
                        }
                    } else {
                        for (w, j) in vweight.data.iter().zip(vjoint.data.iter()) {
                            joint_buf.put_u8(*j);
                            weight_buf.put_f32_le(*w as f32 / 255.0);
                        }
                    }
                }
                wbuffers.push(BufferContainer::Bytes(weight_buf.freeze()));
                ibuffers.push(BufferContainer::Bytes(joint_buf.freeze()));
            }
        }
        for (i, b) in wbuffers.into_iter().enumerate() {
            self.put_attr(
                b,
                json::accessor::Type::Vec4,
                json::accessor::ComponentType::F32,
                None,
                Some(json::mesh::Semantic::Weights(i as u32)),
                None,
                None,
                None,
            );
        }

        for (i, b) in ibuffers.into_iter().enumerate() {
            self.put_attr(
                b,
                json::accessor::Type::Vec4,
                json::accessor::ComponentType::U8,
                None,
                Some(json::mesh::Semantic::Joints(i as u32)),
                None,
                None,
                None,
            );
        }
    }

    fn put_joint_nodes(&mut self, cfg: JointOption) {
        let mut joints_vec: Vec<RdJoint> = self.rdm.joints.clone().unwrap();
        let mut invbind_buf = BytesMut::with_capacity(64 * joints_vec.len());

        let mut global_bind_matrices = Vec::with_capacity(joints_vec.len());
        let mut global_inverse_bind_matrices = Vec::with_capacity(joints_vec.len());

        // Build the inverseBindMatrices
        {
            for joint in &joints_vec {
                let child_quaternion = joint.quaternion;

                let rx = child_quaternion[0];
                let ry = child_quaternion[1];
                let rz = child_quaternion[2];
                let rw = child_quaternion[3];

                let q = Quaternion::new(rw, rx, ry, rz);
                let uq = UnitQuaternion::from_quaternion(q);

                let child_trans = joint.transition;
                let tx = child_trans[0];
                let ty = child_trans[1];
                let tz = child_trans[2];

                let ct: Translation3<f32> = Translation3::new(tx, ty, tz);

                // global transform matrix = T * R * S // S = Identity
                let bindmat: Matrix4<f32> = (ct.to_homogeneous()) * (uq.to_homogeneous());
                global_bind_matrices.push(bindmat);

                // this matrix is the inverse of the global transform of the respective joint, in its initial configuration.
                let mut inv_bindmat: Matrix4<f32> = bindmat.try_inverse().unwrap();
                // why is inv_bindmat.m44 not always 1.0 ?
                // ACCESSOR_INVALID_IBM	Matrix element at index ... (component index 15) contains invalid value: 0.9999998807907104.
                inv_bindmat.m44 = 1.0;
                global_inverse_bind_matrices.push(inv_bindmat);

                // Write the values by iterating through this matrix in column-major order.
                for value in inv_bindmat.iter() {
                    invbind_buf.put_f32_le(*value);
                }
            }
        }

        let buff_freezed = invbind_buf.freeze();
        let mat_accessor_idx = self.put_attr(
            BufferContainer::Bytes(buff_freezed),
            json::accessor::Type::Mat4,
            json::accessor::ComponentType::F32,
            Some(joints_vec.len() as u32),
            None,
            None,
            None,
            None,
        );

        // Convert from
        // rdm: a child joint knows their parent to
        // gltf parents know all their children
        let mut child_list: Vec<Option<Vec<_>>> = vec![None; joints_vec.len()];

        // the rdm model file stores global space transforms.
        // in gltf all transforms are relative to there parent nodes
        let children_of_root_node: Vec<json::root::Index<_>> = match cfg {
            JointOption::ResolveAllRoot => {
                // ResolveParentNode is for debugging purposes
                // no hierarchy ->  every joint will be set as a child of the root node
                (0..joints_vec.len() as u32).map(json::Index::new).collect()
            }
            JointOption::ResolveParentNode => {
                let mut children_of_root_node: Vec<json::root::Index<_>> = Vec::new();
                for (i, (joint, c_bindmat)) in joints_vec
                    .iter_mut()
                    .zip(global_bind_matrices.iter())
                    .enumerate()
                {
                    if joint.parent == 255 || joint.locked {
                        children_of_root_node.push(json::Index::new(i as u32));
                        // Skip to the next iteration because without a parent: global transform == local transform
                        continue;
                    } else {
                        let parent = &mut child_list[joint.parent as usize];
                        match parent {
                            Some(v) => v.push(gltf::json::Index::new(i as u32)),
                            None => *parent = Some(vec![gltf::json::Index::new(i as u32)]),
                        }
                    }
                    let p_inverse_bindmat = &global_inverse_bind_matrices[joint.parent as usize];
                    // "subtract" the parent transform from the current joint's global transform to get the joint's local transform
                    // apply the inverse of the parent's global transform to current joint's global transform. right to left.
                    let mut local = p_inverse_bindmat * c_bindmat;
                    local.m44 = 1.0;
                    let similarity: Similarity3<f32> = nalgebra::try_convert(local).unwrap();
                    debug!("similarity.scaling: {}", similarity.scaling());

                    let isometry: Isometry3<f32> = similarity.isometry;

                    let uqc = isometry.rotation.coords;

                    joint.quaternion = [uqc.x, uqc.y, uqc.z, uqc.w];

                    let trans_point = isometry.translation;
                    joint.transition = [trans_point.x, trans_point.y, trans_point.z];
                }
                children_of_root_node
            }
        };

        let mut skin_nodes: Vec<json::Node> = Vec::new();
        for (joint, children) in joints_vec.iter().zip(child_list) {
            let ijoint = json::Node {
                camera: None,
                children,
                extensions: None,
                extras: None,
                matrix: None,
                mesh: None,
                name: Some(String::from(&joint.name)),
                rotation: Some(json::scene::UnitQuaternion(joint.quaternion)),
                scale: None,
                translation: Some(joint.transition),
                skin: None,
                weights: None,
            };
            skin_nodes.push(ijoint);
        }

        self.skin = Some(json::Skin {
            joints: {
                // includes all nodes except the mesh root node
                let nodes_count_excluding_root_node = skin_nodes.len() as u32;
                (0..nodes_count_excluding_root_node)
                    .map(json::Index::new)
                    .collect()
            },
            extensions: None,
            inverse_bind_matrices: Some(json::Index::new(mat_accessor_idx)),
            skeleton: None,
            extras: None,
            name: None,
        });

        let root_node = json::Node {
            camera: None,
            children: Some(children_of_root_node),
            extensions: None,
            extras: None,
            matrix: None,
            mesh: Some(json::Index::new(0)),
            name: Some(String::from("armature")),
            rotation: None,
            scale: None,
            translation: None,
            skin: Some(json::Index::new(0)),
            weights: None,
        };
        skin_nodes.push(root_node);

        self.nodes = skin_nodes;
    }

    fn put_vertex(&mut self) {
        let mut triangle_vertices: Vec<Vertex> =
            Vec::with_capacity(3 * 4 * self.rdm.vertex.vertex_count as usize);
        let mut min: Vec<f32> = vec![f32::MAX, f32::MAX, f32::MAX];
        let mut max: Vec<f32> = vec![f32::MIN, f32::MIN, f32::MIN];

        for p4h in self.rdm.vertex.iter::<P4h, P3f>(0).unwrap() {
            let x = p4h.data[0];
            let y = p4h.data[1];
            let z = p4h.data[2];

            min[0] = x.min(min[0]);
            min[1] = y.min(min[1]);
            min[2] = z.min(min[2]);

            max[0] = x.max(max[0]);
            max[1] = y.max(max[1]);
            max[2] = z.max(max[2]);

            let t = Vertex {
                position: [x, y, z],
            };
            triangle_vertices.push(t);
        }
        let amin = Some(json::Value::from(min));
        let amax = Some(json::Value::from(max));
        // single vertex buffer
        self.put_attr(
            BufferContainer::Vertex(triangle_vertices),
            json::accessor::Type::Vec3,
            json::accessor::ComponentType::F32,
            None,
            Some(json::mesh::Semantic::Positions),
            amin,
            amax,
            Some(Valid(json::buffer::Target::ArrayBuffer)),
        );
    }

    fn put_tex(&mut self) {
        let mut tbuffers = Vec::new();
        if let Some(iter) = self.rdm.vertex.iter::<T2h, T2f>(0) {
            let mut buff = BytesMut::with_capacity(2 * 4 * self.rdm.vertex.vertex_count as usize);

            for t2h in iter {
                buff.put_f32_le(t2h.data[0]);
                buff.put_f32_le(t2h.data[1]);
            }
            tbuffers.push(BufferContainer::Bytes(buff.freeze()));
        }
        for (i, b) in tbuffers.into_iter().enumerate() {
            self.put_attr(
                b,
                json::accessor::Type::Vec2,
                json::accessor::ComponentType::F32,
                None,
                Some(json::mesh::Semantic::TexCoords(i as u32)),
                None,
                None,
                None,
            );
        }
    }

    fn put_material(&mut self) {
        let material_len = MeshInstance::get_max_material(&self.rdm.mesh_info) as usize + 1;
        // get_max_material returns the max value used to index the material vec
        let mut texture_info_descriptors = vec![None; material_len];
        if let Some(mats) = self.rdm.mat.as_ref() {
            for (i, (image_path, dst)) in mats
                .into_iter()
                .zip(texture_info_descriptors.iter_mut())
                .enumerate()
            {
                let sampler = Default::default();
                self.sampler_vec.push(sampler);

                let fname = image_path.file_stem().unwrap().to_str().unwrap();
                let image = json::Image {
                    uri: Some(format!("{}{}", fname, ".PNG")),
                    buffer_view: None,
                    mime_type: None,
                    extensions: None,
                    extras: None,
                    name: None,
                };
                self.image_vec.push(image);

                let texture = json::Texture {
                    sampler: Some(json::Index::new(i as u32)),
                    source: json::Index::new(i as u32),
                    extensions: None,
                    extras: None,
                    name: None,
                };
                self.texture_vec.push(texture);

                *dst = Some(gltf::json::texture::Info {
                    index: json::Index::new(i as u32),
                    tex_coord: 0,
                    extensions: None,
                    extras: None,
                });
            }
        }

        let mut material_idx_vec = Vec::with_capacity(material_len);
        assert!(self.material_vec.is_empty());
        for itex in texture_info_descriptors {
            let pbr = json::material::PbrMetallicRoughness {
                base_color_texture: itex,
                ..Default::default()
            };

            let map = json::Material {
                alpha_cutoff: None,
                alpha_mode: Valid(json::material::AlphaMode::Opaque),
                pbr_metallic_roughness: pbr,
                ..Default::default()
            };

            material_idx_vec.push(self.material_vec.len() as u32);

            self.material_vec.push(map);
        }
        assert_eq!(material_idx_vec.len(), material_len);
        self.material_idx = Some(material_idx_vec);
    }

    fn run_dds(&mut self, embed_image_buffer: bool) {
        if let Some(mats) = self.rdm.mat.as_ref() {
            let dir = env::temp_dir();
            mats.run_dds_converter(&dir);

            if embed_image_buffer {
                let tmp_dir = env::temp_dir();
                for e in self.image_vec.iter_mut() {
                    let src_dds_file = tmp_dir.join(e.uri.as_ref().unwrap());
                    let mut f = File::open(src_dds_file).unwrap();
                    // prealloc 10 mebibytes for the png
                    let mut buffer = Vec::with_capacity(10 * 1024 * 1024);

                    // read the whole file
                    f.read_to_end(&mut buffer).unwrap();

                    e.uri = None;
                    e.mime_type = Some(json::image::MimeType("image/png".to_string()));
                    let buffer_view_idx = RdGltfBuilder::put_buffer_and_view(
                        &mut self.obj,
                        BufferContainer::U8(buffer),
                        &mut self.buffers,
                        &mut self.buffer_views,
                        None,
                    );
                    e.buffer_view = Some(json::Index::new(buffer_view_idx));
                }
            }
        }
    }

    fn put_buffer_and_view(
        inner: &mut RdGltf,
        buffer: BufferContainer,
        buffers: &mut Vec<json::Buffer>,
        buffer_views: &mut Vec<json::buffer::View>,
        target: Option<json::validation::Checked<gltf::buffer::Target>>,
    ) -> u32 {
        let buffer_p = inner.push_buffer(buffer);
        let buffer = json::Buffer {
            byte_length: buffer_p.len,
            extensions: Default::default(),
            extras: Default::default(),
            name: None,
            uri: Some(buffer_p.file_name),
        };

        let buffer_view = json::buffer::View {
            buffer: json::Index::new(buffer_p.idx),
            byte_length: buffer.byte_length,
            byte_offset: None,
            byte_stride: None,
            extensions: Default::default(),
            extras: Default::default(),
            name: None,
            target,
        };

        buffers.push(buffer);
        let buffer_view_idx = buffer_views.len() as u32;
        buffer_views.push(buffer_view);

        buffer_view_idx
    }

    #[allow(clippy::too_many_arguments)]
    fn put_attr(
        &mut self,
        buffer: BufferContainer,
        acctype: json::accessor::Type,
        component_type: json::accessor::ComponentType,
        count: Option<u32>,
        semantic: Option<json::mesh::Semantic>,
        amin: Option<json::Value>,
        amax: Option<json::Value>,
        target: Option<json::validation::Checked<gltf::buffer::Target>>,
    ) -> u32 {
        let vattr_len = self.rdm.vertex.len();

        let buffer_views_idx = RdGltfBuilder::put_buffer_and_view(
            &mut self.obj,
            buffer,
            &mut self.buffers,
            &mut self.buffer_views,
            target,
        );

        let normals_acc = json::Accessor {
            buffer_view: Some(json::Index::new(buffer_views_idx)),
            byte_offset: 0,
            count: count.unwrap_or(vattr_len),
            component_type: Valid(json::accessor::GenericComponentType(component_type)),
            extensions: Default::default(),
            extras: Default::default(),
            type_: Valid(acctype),
            min: amin,
            max: amax,
            name: None,
            normalized: false,
            sparse: None,
        };

        let accessors_idx = self.accessors.len() as u32;
        self.accessors.push(normals_acc);

        if let Some(semantic) = semantic {
            self.attr_map
                .insert(Valid(semantic), json::Index::new(accessors_idx));
        }
        accessors_idx
    }

    fn put_normal(&mut self) {
        let mut buff = BytesMut::with_capacity(3 * 4 * self.rdm.vertex.vertex_count as usize);
        if let Some(iter) = self.rdm.vertex.iter::<N4b, N3f>(0) {
            for n4b in iter {
                let n = n4b.normalise().data;
                buff.put_f32_le(n[0]);
                buff.put_f32_le(n[1]);
                buff.put_f32_le(n[2]);
            }
        }
        if !buff.is_empty() {
            let buff_freezed = buff.freeze();
            self.put_attr(
                BufferContainer::Bytes(buff_freezed),
                json::accessor::Type::Vec3,
                json::accessor::ComponentType::F32,
                None,
                Some(json::mesh::Semantic::Normals),
                None,
                None,
                None,
            );
        }
    }

    fn put_tangent(&mut self) {
        let mut buff = BytesMut::with_capacity(3 * 4 * self.rdm.vertex.vertex_count as usize);

        if let Some(iter) = self.rdm.vertex.iter::<G4b, G3f>(0) {
            for g3f in iter {
                let t = g3f.normalise().data;
                buff.put_f32_le(t[0]);
                buff.put_f32_le(t[1]);
                buff.put_f32_le(t[2]);

                buff.put_f32_le(1.0);
                /* TODO is this right ?
                if relative_eq!(g4b.data[3], 1.0f32) {
                    buff.put_f32_le(1.0);
                } else {
                    buff.put_f32_le(-1.0);
                }
                */
            }
        }
        if !buff.is_empty() {
            let buff_freezed = buff.freeze();
            self.put_attr(
                BufferContainer::Bytes(buff_freezed),
                json::accessor::Type::Vec4,
                json::accessor::ComponentType::F32,
                None,
                Some(json::mesh::Semantic::Tangents),
                None,
                None,
                None,
            );
        }
    }

    fn put_idx(&mut self) {
        let mut bytes = Vec::with_capacity(self.rdm.mesh_info.len());
        let mut accessor_idx_meshes = Vec::with_capacity(self.rdm.mesh_info.len());
        for submesh in self.rdm.mesh_info.iter() {
            let mut buff = BytesMut::with_capacity(submesh.index_count as usize);
            let r = (submesh.start_index_location as usize / 3) as usize
                ..((submesh.start_index_location / 3) + submesh.index_count / 3) as usize;
            unsafe { buff.put_slice(self.rdm.triangle_indices[r].align_to::<u8>().1) }
            bytes.push((BufferContainer::Bytes(buff.freeze()), submesh.index_count));
        }
        for b in bytes.into_iter() {
            let acc = self.put_attr(
                b.0,
                json::accessor::Type::Scalar,
                json::accessor::ComponentType::U16,
                Some(b.1),
                None,
                None,
                None,
                None,
            );
            accessor_idx_meshes.push(acc);
        }
        self.idx = Some(accessor_idx_meshes);
    }

    pub fn build(mut self) -> RdGltf {
        let animation = self.anim_node.map_or(Default::default(), |n| vec![n]);

        // put_material must already have been run otherwise this panics!
        let mats = self.material_idx.unwrap();
        let indices_vec = self.idx.unwrap();
        assert_eq!(indices_vec.len(), self.rdm.mesh_info.len());

        let mut triangle_vec = Vec::with_capacity(indices_vec.len());
        for (mesh, idx) in self.rdm.mesh_info.iter().zip(indices_vec.iter()) {
            let primitive = json::mesh::Primitive {
                attributes: self.attr_map.clone(),
                extensions: Default::default(),
                extras: Default::default(),
                indices: Some(json::Index::new(*idx)),
                material: Some(json::Index::new(mats[mesh.material as usize])),
                mode: Valid(json::mesh::Mode::Triangles),
                targets: None,
            };
            triangle_vec.push(primitive);
        }

        let mesh = json::Mesh {
            extensions: Default::default(),
            extras: Default::default(),
            name: None,
            primitives: triangle_vec,
            weights: None,
        };

        // if nodes vec is non empty than put_joint_nodes already added a scene root node to the end
        if self.nodes.is_empty() {
            let scene_root_node = json::Node {
                camera: None,
                children: None,
                extensions: Default::default(),
                extras: Default::default(),
                matrix: None,
                mesh: Some(json::Index::new(0)),
                name: None,
                rotation: None,
                scale: None,
                translation: None,
                skin: None,
                weights: None,
            };
            self.nodes.push(scene_root_node);
        }

        // get index of last node (scene root node)
        let root_node_idx = json::Index::new((self.nodes.len() - 1) as u32);

        let root = json::Root {
            accessors: self.accessors,
            buffers: self.buffers,
            buffer_views: self.buffer_views,
            meshes: vec![mesh],
            nodes: self.nodes,
            scene: Some(json::Index::new(0)),
            scenes: vec![json::Scene {
                extensions: Default::default(),
                extras: Default::default(),
                name: None,
                nodes: vec![root_node_idx],
            }],
            skins: if self.skin.is_some() {
                vec![self.skin.clone().unwrap()]
            } else {
                Default::default()
            },
            animations: animation,
            materials: self.material_vec,
            textures: self.texture_vec,
            images: self.image_vec,
            samplers: self.sampler_vec,
            ..Default::default()
        };

        self.obj.root = Some(root);

        self.obj
    }

    fn merge_buffers(&mut self) {
        let size_merge_buffer = self
            .obj
            .buffers
            .iter()
            .map(|x| x.get_bytes_len_padded())
            .sum();

        debug!("size_merge_buffer: {:#?}", size_merge_buffer);
        let mut combined_vec = vec![0; size_merge_buffer];

        let mut view_off_mapping = Vec::new();
        let mut cnt = 0;
        for v in &self.obj.buffers {
            v.to_writer(&mut combined_vec[cnt..cnt + v.get_bytes_len_padded()])
                .unwrap();
            view_off_mapping.push(cnt as u32);
            cnt += v.get_bytes_len_padded();
        }

        for view in self.buffer_views.iter_mut() {
            let n = view_off_mapping[view.buffer.value()];
            view.byte_offset = Some(view.byte_offset.unwrap_or(0) + n as u32);
            view.buffer = json::Index::new(0);
        }

        self.buffers[0].byte_length = combined_vec.len().try_into().unwrap();
        let padded_combined_vec = BufferContainer::U8(combined_vec);
        debug!(
            "size_merge_buffer: {:#?}",
            padded_combined_vec.get_bytes_len_padded()
        );
        debug!("cnt: {:#?}", cnt);

        self.buffers.truncate(1);
        self.obj.buffers = vec![padded_combined_vec];
    }
}

impl From<RdModell> for RdGltfBuilder {
    fn from(rdm: RdModell) -> Self {
        let has_skin = rdm.has_skin();
        let has_anim = rdm.anim.is_some();

        let mut b = RdGltfBuilder::new(rdm);

        b.put_vertex();
        b.put_idx();

        b.put_tex();
        b.put_material();

        b.put_normal();
        b.put_tangent();

        if has_skin {
            b.put_joint_nodes(JointOption::ResolveParentNode);
            b.put_joint_weight(true);

            if has_anim {
                b.put_rdm_anim();
            }
        }

        b
    }
}

pub fn build(rdm: RdModell, dir: Option<PathBuf>, create_new: bool, config: GltfExportFormat) {
    let mat_opt = rdm.mat.clone();
    let mut b = RdGltfBuilder::from(rdm);
    b.run_dds(config == GltfExportFormat::Glb);
    if config == GltfExportFormat::Glb || config == GltfExportFormat::GltfSeparateMinimise {
        b.merge_buffers();
        if config == GltfExportFormat::Glb {
            b.buffers[0].uri = None;
        }
    }

    let p = b.build();
    info!("gltf build end");
    info!("write_gltf");
    p.write_gltf(dir, mat_opt, create_new, config);
}

struct RdGltf {
    buffers: Vec<BufferContainer>,
    root: Option<json::Root>,
}
enum BufferContainer {
    U8(Vec<u8>),
    Bytes(Bytes),
    Vertex(Vec<Vertex>),
}

impl BufferContainer {
    fn get_bytes_len_real(&self) -> usize {
        self.get_bytes().len()
    }

    fn get_padded_added(&self) -> usize {
        let real_len = self.get_bytes_len_real();
        assert_ne!(real_len, 0);
        if real_len % 4 == 0 {
            0
        } else {
            4 - (real_len % 4)
        }
    }

    fn get_bytes_len_padded(&self) -> usize {
        self.get_bytes_len_real() + self.get_padded_added()
    }

    fn get_bytes(&self) -> &[u8] {
        unsafe {
            match self {
                BufferContainer::U8(v) => v.align_to::<u8>().1,
                BufferContainer::Vertex(vert) => vert.align_to::<u8>().1,
                BufferContainer::Bytes(b) => b,
            }
        }
    }

    fn to_writer(&self, mut writer: impl io::Write) -> std::io::Result<()> {
        static PAD: &[u8] = &[0x0, 0x0, 0x0];
        let bytes = self.get_bytes();
        let to_add = self.get_padded_added();
        writer.write_all(bytes)?;
        if to_add < 4 && to_add != 0 {
            writer.write_all(&PAD[0..to_add])?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GltfExportFormat {
    GltfSeparate,
    GltfSeparateMinimise,
    Glb,
}

impl FromStr for GltfExportFormat {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.to_ascii_lowercase().as_str() {
            "gltf" => Ok(GltfExportFormat::GltfSeparate),
            "gltfm" | "gltfmi" | "gltfmin" => Ok(GltfExportFormat::GltfSeparateMinimise),
            "glb" => Ok(GltfExportFormat::Glb),
            _ => Err(format!(
                "Invalid value for GltfExportFormat: {}, Only gltf/gltfmin/glb are allowed value",
                input
            )),
        }
    }
}

impl RdGltf {
    fn new() -> Self {
        RdGltf {
            buffers: vec![],
            root: None,
        }
    }

    fn write_gltf(
        self,
        dir: Option<PathBuf>,
        optmat: Option<RdMaterial>,
        create_new: bool,
        config: GltfExportFormat,
    ) {
        let mut file = dir.unwrap_or_else(|| {
            let f = PathBuf::from("gltf_out");
            let _ = fs::create_dir(&f);
            f
        });

        if file.is_dir() {
            file.push("out");
        }
        if config == GltfExportFormat::Glb {
            file.set_extension("glb");
        } else {
            file.set_extension("gltf");
        }
        info!("{:?}", file);

        let mut writer = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .create_new(create_new)
            .open(&file)
            .expect("I/O error");

        match config {
            GltfExportFormat::Glb => {
                //TODO fix this. Currently Glb writer ignores these values otherwise this would not work.
                let header: gltf::binary::Header = gltf::binary::Header {
                    magic: Default::default(),
                    version: 2,
                    length: 0xDEAD_BEEF,
                };
                let j = json::serialize::to_vec(&self.root.unwrap()).expect("Serialization error");
                let glb = gltf::Glb {
                    header,
                    json: Cow::from(&j),
                    bin: Some(Cow::from(self.buffers[0].get_bytes())),
                };
                glb.to_writer(writer).expect("I/O error");
                debug!("json: {}", glb.json.len());
                debug!("bin: {}", &self.buffers[0].get_bytes_len_padded());
            }
            _ => {
                let vjson = json::serialize::to_vec_pretty(&self.root.unwrap())
                    .expect("Serialization error");
                writer.write_all(&vjson).expect("I/O error");

                debug!("wrote json to disk!");

                file.pop();
                let udir = file;

                for (i, bin) in self.buffers.into_iter().enumerate() {
                    let mut file_path = udir.clone();
                    file_path.push(format!("buffer{}.bin", i));
                    debug!("write_all {:?}", &file_path);
                    let mut writer = OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .create_new(create_new)
                        .open(&file_path)
                        .expect("I/O error");
                    bin.to_writer(&mut writer).unwrap();
                }

                // copy converted png from tmp to dest
                if let Some(mat) = optmat.as_ref() {
                    let tmp_dir = env::temp_dir();
                    for e in mat.into_iter() {
                        let mut src = tmp_dir.join(e.file_stem().unwrap());
                        src.set_extension("PNG");
                        let mut dst_file = udir.join(e.file_stem().unwrap());
                        dst_file.set_extension("PNG");
                        debug!("copy: {:?} to {:?}", &src, &dst_file);
                        fs::copy(src, &dst_file).unwrap();
                    }
                }
            }
        }
    }

    fn push_buffer(&mut self, b: BufferContainer) -> PushBufferResult {
        let idx = self.buffers.len();

        let file_name = format!("buffer{}.bin", idx);
        let len = b.get_bytes_len_padded();
        self.buffers.push(b);

        PushBufferResult {
            file_name,
            len: len as u32,
            idx: idx as u32,
        }
    }
}

struct PushBufferResult {
    file_name: String,
    len: u32,
    idx: u32,
}
