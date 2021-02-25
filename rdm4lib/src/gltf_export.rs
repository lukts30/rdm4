use gltf::json;

use gltf::json as gltf_json;
use half::f16;
use std::{
    borrow::Cow,
    convert::TryInto,
    fs::{self, OpenOptions},
    io,
    path::PathBuf,
};

use bytes::{BufMut, Bytes, BytesMut};
use json::validation::Checked::Valid;

use std::io::Write;

use crate::{rdm_material::RdMaterial, Triangle};
use crate::{vertex::Normalise, vertex::UniqueIdentifier, RdModell};
use crate::{I4b, Normal, Position, Tangent, Texcoord};
use crate::{RdJoint, W4b};

use nalgebra::*;
use std::collections::VecDeque;

use gltf::mesh::Semantic;
use std::collections::HashMap;

#[derive(Copy, Clone, Debug)]
#[repr(C)]
struct Vertex {
    position: [f32; 3],
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
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
        let time_f32_max = (anim.time_max as f32) / 1000.0;

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
                    panic!(
                        "Could not find animation target {:?} in base model {:?}",
                        &janim.name, p
                    );
                }
            };

            let count = janim.len as usize;

            let rot_start = rot_anim_buf.len();
            let trans_start = trans_anim_buf.len();
            let t_start = t_anim_buf.len();

            for f in &janim.frames {
                rot_anim_buf.put_f32_le(f.rotation[0]);
                rot_anim_buf.put_f32_le(f.rotation[1]);
                rot_anim_buf.put_f32_le(f.rotation[2]);
                rot_anim_buf.put_f32_le(-f.rotation[3]);

                trans_anim_buf.put_f32_le(f.translation[0]);
                trans_anim_buf.put_f32_le(f.translation[1]);
                trans_anim_buf.put_f32_le(f.translation[2]);

                t_anim_buf.put_f32_le(f.time);
            }
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
                max: Some(json::Value::from(vec![time_f32_max])),
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
        let n = self.rdm.vertex.find(UniqueIdentifier::I4b).len();
        let mut ibuffers = Vec::with_capacity(n);
        let mut wbuffers = Vec::with_capacity(n);
        for i in 0..n {
            if let Some(iter) = self.rdm.vertex.iter::<I4b, I4b>(i) {
                let mut weight_buf =
                    BytesMut::with_capacity((4 + 4 * 4) * self.rdm.vertex.len() as usize);
                let mut joint_buf = BytesMut::with_capacity(4 * self.rdm.vertex.len() as usize);

                let w4b_iter: Box<dyn Iterator<Item = W4b> + '_> =
                    match self.rdm.vertex.iter::<W4b, W4b>(i) {
                        Some(i) => Box::new(i),
                        None => Box::new(self.rdm.vertex.w4b_default_iter()),
                    };

                for ((e, w), sum) in iter
                    .zip(w4b_iter)
                    .zip(self.rdm.vertex.weight_sum.as_ref().unwrap())
                {
                    if normalise {
                        // ACCESSOR_JOINTS_USED_ZERO_WEIGHT
                        if w.blend_weight[0] != 0 {
                            joint_buf.put_u8(e.blend_idx[0]);
                        } else {
                            joint_buf.put_u8(0);
                        }
                        if w.blend_weight[1] != 0 {
                            joint_buf.put_u8(e.blend_idx[1]);
                        } else {
                            joint_buf.put_u8(0);
                        }
                        if w.blend_weight[2] != 0 {
                            joint_buf.put_u8(e.blend_idx[2]);
                        } else {
                            joint_buf.put_u8(0);
                        }
                        if w.blend_weight[3] != 0 {
                            joint_buf.put_u8(e.blend_idx[3]);
                        } else {
                            joint_buf.put_u8(0);
                        }

                        // ACCESSOR_WEIGHTS_NON_NORMALIZED
                        let sum_float: f32 = 255.0f32 / *sum as f32;

                        weight_buf.put_f32_le(w.blend_weight[0] as f32 / 255.0 * sum_float); // > 0.0
                        weight_buf.put_f32_le(w.blend_weight[1] as f32 / 255.0 * sum_float); // 0.0
                        weight_buf.put_f32_le(w.blend_weight[2] as f32 / 255.0 * sum_float); // 0.0
                        weight_buf.put_f32_le(w.blend_weight[3] as f32 / 255.0 * sum_float);
                    } else {
                        joint_buf.put_u8(e.blend_idx[0]);
                        joint_buf.put_u8(e.blend_idx[1]);
                        joint_buf.put_u8(e.blend_idx[2]);
                        joint_buf.put_u8(e.blend_idx[3]);

                        weight_buf.put_f32_le(w.blend_weight[0] as f32 / 255.0); // > 0.0
                        weight_buf.put_f32_le(w.blend_weight[1] as f32 / 255.0); // 0.0
                        weight_buf.put_f32_le(w.blend_weight[2] as f32 / 255.0); // 0.0
                        weight_buf.put_f32_le(w.blend_weight[3] as f32 / 255.0);
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

        // inverseBindMatrices
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

                // global transform matrix = T * R * S
                let bindmat = (ct.to_homogeneous()) * (uq.to_homogeneous()) * Matrix4::identity();

                // matrix is the inverse of the global transform of the respective joint, in its initial configuration.
                let inv_bindmat = bindmat.try_inverse().unwrap();

                //column-major order
                invbind_buf.put_f32_le(inv_bindmat.m11);
                invbind_buf.put_f32_le(inv_bindmat.m21);
                invbind_buf.put_f32_le(inv_bindmat.m31);
                invbind_buf.put_f32_le(inv_bindmat.m41);

                invbind_buf.put_f32_le(inv_bindmat.m12);
                invbind_buf.put_f32_le(inv_bindmat.m22);
                invbind_buf.put_f32_le(inv_bindmat.m32);
                invbind_buf.put_f32_le(inv_bindmat.m42);

                invbind_buf.put_f32_le(inv_bindmat.m13);
                invbind_buf.put_f32_le(inv_bindmat.m23);
                invbind_buf.put_f32_le(inv_bindmat.m33);
                invbind_buf.put_f32_le(inv_bindmat.m43);

                invbind_buf.put_f32_le(inv_bindmat.m14);
                invbind_buf.put_f32_le(inv_bindmat.m24);
                invbind_buf.put_f32_le(inv_bindmat.m34);
                invbind_buf.put_f32_le(1.0f32);

                // why is inv_bindmat.m44 not always 1.0 ?
                // ACCESSOR_INVALID_IBM	Matrix element at index 143 (component index 15) contains invalid value: 0.9999998807907104.
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

        let mut skin_nodes: Vec<json::Node> = Vec::new();

        let mut arm: Vec<json::root::Index<_>> = Vec::new();

        for (i, joint) in joints_vec.iter_mut().enumerate() {
            if joint.parent == 255 || cfg == JointOption::ResolveAllRoot {
                joint.locked = true;
                arm.push(json::Index::new(i as u32));
            }
        }

        let main_node = json::Node {
            camera: None,
            children: Some(arm),
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

        let jlen = joints_vec.len();

        let mut tb_rel: VecDeque<(usize, usize)> = VecDeque::new();

        let mut child_list: VecDeque<_> = VecDeque::new();
        for z in 0..jlen {
            let mut child: Vec<gltf_json::root::Index<_>> = Vec::new();
            for j in 0..jlen {
                if joints_vec[j].parent == z as u8 && joints_vec[z].locked && !joints_vec[j].locked
                {
                    joints_vec[j].locked = true;
                    child.push(gltf_json::Index::new(j as u32));
                    tb_rel.push_back((z, j));
                }
            }

            if !child.is_empty() && cfg == JointOption::ResolveParentNode {
                child_list.push_back(Some(child))
            } else {
                child_list.push_back(None);
            }
        }

        // the rdm model file stores global space transforms.
        // in gltf all transforms are relative to there parent nodes
        while !tb_rel.is_empty() && cfg == JointOption::ResolveParentNode {
            let target = tb_rel.pop_back().unwrap();

            let master_idx = target.0;
            let child_idx = target.1;

            let master_trans = joints_vec[master_idx].transition;
            let mx = master_trans[0];
            let my = master_trans[1];
            let mz = master_trans[2];

            let master_quaternion = joints_vec[master_idx].quaternion;

            let mqx = master_quaternion[0];
            let mqy = master_quaternion[1];
            let mqz = master_quaternion[2];
            let mqw = master_quaternion[3];

            let mq = Quaternion::new(mqw, mqx, mqy, mqz);
            let muq = UnitQuaternion::from_quaternion(mq);

            let child_quaternion = joints_vec[child_idx].quaternion;

            let rx = child_quaternion[0];
            let ry = child_quaternion[1];
            let rz = child_quaternion[2];
            let rw = child_quaternion[3];

            let q = Quaternion::new(rw, rx, ry, rz);
            let uq = UnitQuaternion::from_quaternion(q);

            let rel_uq = (muq.inverse()) * uq;
            let uqc = rel_uq.quaternion().coords;

            joints_vec[child_idx].quaternion = [uqc.x, uqc.y, uqc.z, uqc.w];

            let child_trans = joints_vec[child_idx].transition;
            let tx = child_trans[0];
            let ty = child_trans[1];
            let tz = child_trans[2];

            let mt: Translation3<f32> = Translation3::new(mx, my, mz).inverse();
            let ct: Translation3<f32> = Translation3::new(tx, ty, tz).inverse();

            let nx = ct.x - mt.x;
            let ny = ct.y - mt.y;
            let nz = ct.z - mt.z;

            let trans_inter_point = Point3::new(nx, ny, nz);

            let uik = muq.inverse_transform_point(&trans_inter_point);

            let uik_x = uik.x;
            let uik_y = uik.y;
            let uik_z = uik.z;

            let trans_point = Translation3::new(uik_x, uik_y, uik_z).inverse();
            joints_vec[child_idx].transition = [trans_point.x, trans_point.y, trans_point.z];
        }

        for joint in &joints_vec {
            let ijoint = json::Node {
                camera: None,
                children: { child_list.pop_front().unwrap() },
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

        skin_nodes.push(main_node);
        let nodes_count = skin_nodes.len() - 1;
        self.nodes.append(&mut skin_nodes);

        // skin root node
        let mut joint_indi_vec: Vec<json::root::Index<_>> = Vec::new();
        for i in 0..nodes_count {
            joint_indi_vec.push(json::Index::new(i as u32));
        }

        self.skin = Some(json::Skin {
            joints: joint_indi_vec,
            extensions: None,
            inverse_bind_matrices: Some(json::Index::new(mat_accessor_idx)),
            skeleton: None,
            extras: None,
            name: None,
        });
    }

    fn put_vertex(&mut self) {
        let mut triangle_vertices: Vec<Vertex> =
            Vec::with_capacity(3 * 4 * self.rdm.vertex.vertex_count as usize);
        let mut min: Vec<f32> = vec![f32::MAX, f32::MAX, f32::MAX];
        let mut max: Vec<f32> = vec![f32::MIN, f32::MIN, f32::MIN];

        for p4h in self
            .rdm
            .vertex
            .iter::<Position<f16>, Position<f32>>(0)
            .unwrap()
        {
            let x = p4h.pos[0];
            let y = p4h.pos[1];
            let z = p4h.pos[2];

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
        if let Some(iter) = self.rdm.vertex.iter::<Texcoord<f16>, Texcoord<f32>>(0) {
            let mut buff = BytesMut::with_capacity(2 * 4 * self.rdm.vertex.vertex_count as usize);

            for t2h in iter {
                buff.put_f32_le(t2h.tex[0]);
                buff.put_f32_le(t2h.tex[1]);
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
        let mut info_vec = Vec::new();
        if let Some(mats) = self.rdm.mat.as_ref() {
            for (i, mat) in mats.c_model_diff_tex.iter().enumerate() {
                let sampler = json::texture::Sampler {
                    ..Default::default()
                };
                self.sampler_vec.push(sampler);

                let fname = mat.file_stem().unwrap().to_str().unwrap();
                let image = json::Image {
                    uri: Some(format!("{}{}", fname, ".png")),
                    buffer_view: None,
                    mime_type: None,
                    extensions: None,
                    extras: None,
                    name: None,
                };
                self.image_vec.push(image);

                // TODO DO NOT USE i
                let texture = json::Texture {
                    sampler: Some(json::Index::new(i as u32)),
                    source: json::Index::new(i as u32),
                    extensions: None,
                    extras: None,
                    name: None,
                };
                self.texture_vec.push(texture);

                info_vec.push(Some(gltf_json::texture::Info {
                    index: json::Index::new(i as u32),
                    tex_coord: 0,
                    extensions: None,
                    extras: None,
                }));
            }
        }

        //assert_eq!(self.rdm.mesh_info.len(), info_vec.len());
        let mut max_mesh: usize = 0;
        for m in self.rdm.mesh_info.iter() {
            max_mesh = max_mesh.max(m.mesh as usize);
        }

        while max_mesh + 1 > info_vec.len() {
            info_vec.push(None);
        }
        debug!("{:#?} {:#?}", max_mesh, info_vec.len());

        let mut material_idx_vec = Vec::new();
        for itex in info_vec {
            let pbr = json::material::PbrMetallicRoughness {
                base_color_texture: itex,
                ..Default::default()
            };

            let map = json::Material {
                // FALSE WARNING
                // MATERIAL_ALPHA_CUTOFF_INVALID_MODE
                // This value is ignored for other modes.
                alpha_cutoff: json::material::AlphaCutoff(0.0f32),
                alpha_mode: Valid(json::material::AlphaMode::Opaque),
                pbr_metallic_roughness: pbr,
                ..Default::default()
            };

            material_idx_vec.push(self.material_vec.len() as u32);

            self.material_vec.push(map);
        }
        self.material_idx = Some(material_idx_vec);
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

        let buffer_p = self.obj.push_buffer(buffer);
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

        self.buffers.push(buffer);
        let buffer_views_idx = self.buffer_views.len() as u32;
        self.buffer_views.push(buffer_view);

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
        if let Some(iter) = self.rdm.vertex.iter::<Normal<u8>, Normal<f32>>(0) {
            for n4b in iter {
                let n = n4b.normalise().normals;
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

        if let Some(iter) = self.rdm.vertex.iter::<Tangent<u8>, Tangent<f32>>(0) {
            for g4b in iter {
                let t = g4b.normalise().tangent;
                buff.put_f32_le(t[0]);
                buff.put_f32_le(t[1]);
                buff.put_f32_le(t[2]);

                // TODO is this right ?
                if relative_eq!(g4b.tangent[3], 1.0f32) {
                    buff.put_f32_le(1.0);
                } else {
                    buff.put_f32_le(-1.0);
                }
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
        // Indexed triangle list
        let triangle_idx: Vec<Triangle> = self.rdm.triangle_indices.clone();
        trace!("triangle_idx.len: {}", triangle_idx.len());

        let triangle_idx_p = self
            .obj
            .push_buffer(BufferContainer::Triangle(triangle_idx));
        let triangle_idx_len_b = triangle_idx_p.len as u32;

        let buffer_idx = json::Buffer {
            byte_length: triangle_idx_len_b,
            extensions: Default::default(),
            extras: Default::default(),
            name: None,
            uri: Some(triangle_idx_p.file_name),
        };
        self.buffers.push(buffer_idx);

        let buffer_idx_view = json::buffer::View {
            buffer: json::Index::new(triangle_idx_p.idx),
            byte_length: triangle_idx_len_b,
            byte_offset: None,
            byte_stride: None,
            extensions: Default::default(),
            extras: Default::default(),
            name: None,
            target: Some(Valid(json::buffer::Target::ElementArrayBuffer)),
        };

        self.buffer_views.push(buffer_idx_view);

        let buffer_idx_view_idx = (self.buffer_views.len() - 1) as u32;

        let mut accessor_idx_meshes = Vec::new();
        let mut sum = 0;
        for submesh in self.rdm.mesh_info.iter() {
            let idx = json::Accessor {
                buffer_view: Some(json::Index::new(buffer_idx_view_idx)),
                byte_offset: submesh.start_index_location * 2,
                count: submesh.index_count,
                component_type: Valid(json::accessor::GenericComponentType(
                    json::accessor::ComponentType::U16,
                )),
                extensions: Default::default(),
                extras: Default::default(),
                type_: Valid(json::accessor::Type::Scalar),
                min: None,
                max: None,
                name: None,
                normalized: false,
                sparse: None,
            };

            self.accessors.push(idx);

            let accessors_idx = (self.accessors.len() - 1) as u32;
            accessor_idx_meshes.push(accessors_idx);
            sum += submesh.index_count;
        }
        assert!((sum * 2..4 + sum * 2).contains(&triangle_idx_p.len));
        self.idx = Some(accessor_idx_meshes);
    }

    pub fn build(mut self) -> RdGltf {
        let animation = if self.anim_node.is_some() {
            vec![self.anim_node.unwrap()]
        } else {
            Default::default()
        };

        let mut triangle_vec = Vec::new();
        for (_i, ((submesh, idx), _mesh_idx)) in self
            .rdm
            .mesh_info
            .iter()
            .zip(self.idx.unwrap())
            .zip(self.material_idx.unwrap())
            .enumerate()
        {
            //assert_eq!(i as u32, submesh.mesh);
            let primitive = json::mesh::Primitive {
                attributes: self.attr_map.clone(),
                extensions: Default::default(),
                extras: Default::default(),
                indices: Some(json::Index::new(idx)),
                material: Some(json::Index::new(submesh.mesh)),
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

        let node_def = json::Node {
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

        //TODO better check
        if self.nodes.is_empty() {
            self.nodes.push(node_def);
        }

        //TODO : must be root node
        let node_len = json::Index::new((self.nodes.len() - 1) as u32);

        let root = json::Root {
            accessors: self.accessors,
            buffers: self.buffers,
            buffer_views: self.buffer_views,
            meshes: vec![mesh],
            nodes: self.nodes,
            scenes: vec![json::Scene {
                extensions: Default::default(),
                extras: Default::default(),
                name: None,
                nodes: vec![node_len],
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
    Triangle(Vec<Triangle>),
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
                BufferContainer::Triangle(tri) => tri.align_to::<u8>().1,
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

                if let Some(mat) = optmat.as_ref() {
                    mat.run_dds_converter(&udir);
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
