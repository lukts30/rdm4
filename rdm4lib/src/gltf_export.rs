use gltf::json;

use gltf::json as gltf_json;

use std::{fs, mem};

use bytes::{BufMut, BytesMut};
use json::validation::Checked::Valid;

use std::io::Write;

use crate::RDJoint;
use crate::RDModell;
use crate::Triangle;
use crate::VertexFormat;

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

fn to_padded_byte_vector<T>(vec: Vec<T>) -> Vec<u8> {
    let byte_length = vec.len() * mem::size_of::<T>();
    let byte_capacity = vec.capacity() * mem::size_of::<T>();
    let alloc = vec.into_boxed_slice();
    let ptr = Box::<[T]>::into_raw(alloc) as *mut u8;
    let mut new_vec = unsafe { Vec::from_raw_parts(ptr, byte_length, byte_capacity) };
    while new_vec.len() % 4 != 0 {
        new_vec.push(0); // pad to multiple of four bytes
    }
    new_vec
}
struct RDGltfBuilder {
    #[allow(dead_code)]
    name: Option<String>,
    buffers: Vec<json::Buffer>,
    buffer_views: Vec<json::buffer::View>,
    accessors: Vec<json::Accessor>,
    nodes: Vec<json::Node>,
    attr_map: HashMap<json::validation::Checked<Semantic>, json::Index<json::Accessor>>,
    idx: Option<u32>,

    rdm: RDModell,
    obj: RDGltf, // private
    skin: Option<json::Skin>,
    anim_node: Option<json::Animation>,
}

impl RDGltfBuilder {
    fn new(rdm: RDModell) -> Self {
        RDGltfBuilder {
            name: None,
            buffers: Vec::new(),
            buffer_views: Vec::new(),
            accessors: Vec::new(),
            nodes: Vec::new(),
            attr_map: HashMap::new(),
            rdm,
            obj: RDGltf::new(),
            skin: None,
            idx: None,
            anim_node: None,
        }
    }

    pub fn put_rdm_anim(&mut self, buffv_idx: u32, mut acc_idx: u32) {
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

        // alloc one buffer
        // vec buffer_v
        let mut buffer_v_vec = Vec::new();

        // vec acc
        let mut acc_vec = Vec::new();

        // ** animations

        //let buffv_idx = 7;
        //let mut acc_idx = 8;

        // anim node
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
            let target_node_idx = *modell_nodes.get(&janim.name).unwrap() as u32;

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
                buffer_view: Some(json::Index::new(acc_idx)),
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
                buffer_view: Some(json::Index::new(acc_idx)),
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
                buffer_view: Some(json::Index::new(acc_idx)),
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

        let _ = fs::create_dir("gltf_out");
        let mut writer = fs::File::create("gltf_out/buffer10.bin").expect("I/O error");

        writer.write_all(&rot_anim_buf).expect("I/O error");
        writer.write_all(&trans_anim_buf).expect("I/O error");
        writer.write_all(&t_anim_buf).expect("I/O error");

        let anim_buffer = json::Buffer {
            byte_length: (rot_anim_buf.len() + trans_anim_buf.len() + t_anim_buf.len()) as u32,
            extensions: Default::default(),
            extras: Default::default(),
            name: None,
            uri: Some("buffer10.bin".into()),
        };

        self.buffers.push(anim_buffer);

        self.buffer_views.append(&mut buffer_v_vec);

        self.accessors.append(&mut acc_vec);

        self.anim_node = Some(anim_node);
    }

    fn put_joint_weight(&mut self) {
        let input_vec = &self.rdm.vertices;

        let mut joint_weight_buf = BytesMut::with_capacity((4 + 4 * 4) * input_vec.len());
        let mut weight: [f32; 4] = [1.0, 0.0, 0.0, 0.0];

        for vert in input_vec {
            match vert {
                VertexFormat::P4h_N4b_T2h_I4b(_, _, _, i4b)
                | VertexFormat::P4h_N4b_G4b_B4b_T2h_I4b(_, _, _, _, _, i4b) => {
                    /* 'ACCESSOR_JOINTS_USED_ZERO_WEIGHT'
                    Must only have one joint/blend_idx since the others are zero weight
                    only problematic if gltf -> lossy rdm target while keeping all blend_idx from gltf -> gltf
                    TODO: do not write all idx in rdm_writer  */

                    joint_weight_buf.put_u8(i4b.blend_idx[0]);
                    joint_weight_buf.put_u8(0);
                    joint_weight_buf.put_u8(0);
                    joint_weight_buf.put_u8(0);

                    joint_weight_buf.put_f32_le(weight[0]); // > 0.0
                    joint_weight_buf.put_f32_le(weight[1]); // 0.0
                    joint_weight_buf.put_f32_le(weight[2]); // 0.0
                    joint_weight_buf.put_f32_le(weight[3]); // 0.0
                }
                VertexFormat::P4h_N4b_T2h_I4b_W4b(_, _, _, i4b, w4b) => {
                    weight = [
                        w4b.blend_weight[0] as f32 / 255.0,
                        w4b.blend_weight[1] as f32 / 255.0,
                        w4b.blend_weight[2] as f32 / 255.0,
                        w4b.blend_weight[3] as f32 / 255.0,
                    ];

                    joint_weight_buf.put_slice(&i4b.blend_idx);
                    joint_weight_buf.put_f32_le(weight[0]);
                    joint_weight_buf.put_f32_le(weight[1]);
                    joint_weight_buf.put_f32_le(weight[2]);
                    joint_weight_buf.put_f32_le(weight[3]);
                }

                _ => panic!("not supported !"),
            };
        }

        let real_len = joint_weight_buf.len() as u32;
        joint_weight_buf.put_u32_le(0);

        let jw_buffer_p = self.obj.push_buffer(joint_weight_buf.to_vec());

        let jw_buffer = json::Buffer {
            byte_length: joint_weight_buf.len() as u32,
            extensions: Default::default(),
            extras: Default::default(),
            name: None,
            uri: Some(jw_buffer_p.file_name.clone()),
        };

        self.buffers.push(jw_buffer);

        let joint_buffer_view = json::buffer::View {
            buffer: json::Index::new(jw_buffer_p.idx),
            byte_length: real_len,
            byte_offset: None,
            byte_stride: Some(20),
            extensions: Default::default(),
            extras: Default::default(),
            name: None,
            target: None,
        };
        self.buffer_views.push(joint_buffer_view);
        let joint_buffer_view_idx = (self.buffer_views.len() - 1) as u32;

        let joint_accessor = json::Accessor {
            buffer_view: Some(json::Index::new(joint_buffer_view_idx)),
            byte_offset: 0,
            count: input_vec.len() as u32,
            component_type: Valid(json::accessor::GenericComponentType(
                json::accessor::ComponentType::U8,
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
        self.accessors.push(joint_accessor);
        let joint_accessor_idx = (self.accessors.len() - 1) as u32;

        let weight_buffer_view = json::buffer::View {
            buffer: json::Index::new(jw_buffer_p.idx),
            byte_length: real_len,
            byte_offset: Some(4),
            byte_stride: Some(20),
            extensions: Default::default(),
            extras: Default::default(),
            name: None,
            target: None,
        };
        self.buffer_views.push(weight_buffer_view);
        let weight_buffer_view_idx = (self.buffer_views.len() - 1) as u32;

        let weight_accessor = json::Accessor {
            buffer_view: Some(json::Index::new(weight_buffer_view_idx)),
            byte_offset: 0,
            count: input_vec.len() as u32,
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
        self.accessors.push(weight_accessor);
        let weight_accessor_idx = (self.accessors.len() - 1) as u32;

        self.attr_map.insert(
            Valid(json::mesh::Semantic::Joints(0)),
            json::Index::new(joint_accessor_idx),
        );
        self.attr_map.insert(
            Valid(json::mesh::Semantic::Weights(0)),
            json::Index::new(weight_accessor_idx),
        );
    }

    fn put_joint_nodes(&mut self, cfg: JointOption) {
        let mut joints_vec: Vec<RDJoint> = self.rdm.joints.clone().unwrap();
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
                let invbind_buf_len = invbind_buf.len();

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
                // assert_relative_eq!(1.0, inv_bindmat.m44)
                // assert_relative_eq!(1.0, 0.9999998807907104f32) => true but
                // ACCESSOR_INVALID_IBM	Matrix element at index 143 (component index 15) contains invalid value: 0.9999998807907104.
                let invbind_buf_written = invbind_buf.len() - invbind_buf_len;
                assert_eq!(invbind_buf_written, 64);
            }
        }
        let invbind_buf_p = self.obj.push_buffer(invbind_buf.to_vec());

        let mat_buffer = json::Buffer {
            byte_length: invbind_buf.len() as u32,
            extensions: Default::default(),
            extras: Default::default(),
            name: None,
            uri: Some(invbind_buf_p.file_name),
        };

        let mat_buffer_view = json::buffer::View {
            buffer: json::Index::new(invbind_buf_p.idx),
            byte_length: invbind_buf.len() as u32,
            byte_offset: None,
            byte_stride: None,
            extensions: Default::default(),
            extras: Default::default(),
            name: None,
            target: None,
        };

        let mut mat_accessor = json::Accessor {
            buffer_view: Some(json::Index::new(2)),
            byte_offset: 0,
            count: joints_vec.len() as u32,
            component_type: Valid(json::accessor::GenericComponentType(
                json::accessor::ComponentType::F32,
            )),
            extensions: Default::default(),
            extras: Default::default(),
            type_: Valid(json::accessor::Type::Mat4),
            min: None,
            max: None,
            name: None,
            normalized: false,
            sparse: None,
        };

        // end inverseBindMatrices
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

            //

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

            //

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

        self.buffers.push(mat_buffer);
        self.buffer_views.push(mat_buffer_view);

        let mat_buffer_view_idx = (self.buffer_views.len() - 1) as u32;

        mat_accessor.buffer_view = Some(json::Index::new(mat_buffer_view_idx));
        self.accessors.push(mat_accessor);
        let mat_accessor_idx = (self.accessors.len() - 1) as u32;

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

    fn get_skins_or_default(&self) -> Vec<json::Skin> {
        if self.skin.is_some() {
            vec![self.skin.clone().unwrap()]
        } else {
            Default::default()
        }
    }

    fn rdm_vertex_to_gltf(rdm: &RDModell) -> (Vec<Vertex>, Vec<f32>, Vec<f32>) {
        let input_vec = &rdm.vertices;

        let mut out: Vec<Vertex> = Vec::new();

        //TODO FIXME arbitrarily chosen
        let mut min: Vec<f32> = vec![100.0, 100.0, 100.0];
        let mut max: Vec<f32> = vec![-100.0, -100.0, -100.0];

        for vert in input_vec {
            let p4h = vert.get_p4h();
            let x = p4h.pos[0].to_f32();
            let y = p4h.pos[1].to_f32();
            let z = p4h.pos[2].to_f32();

            min[0] = x.min(min[0]);
            min[1] = y.min(min[1]);
            min[2] = z.min(min[2]);

            max[0] = x.max(max[0]);
            max[1] = y.max(max[1]);
            max[2] = z.max(max[2]);

            let t = Vertex {
                position: [x, y, z],
            };
            out.push(t);
        }
        (out, min, max)
    }

    fn put_vertex(&mut self) {
        let conv = RDGltfBuilder::rdm_vertex_to_gltf(&self.rdm);

        let triangle_vertices = conv.0;
        let min = conv.1;
        let max = conv.2;
        let triangle_len = triangle_vertices.len() as u32;

        // single vertex buffer

        let buffer_p = self.obj.push_buffer(triangle_vertices);
        let buffer_length = buffer_p.len as u32;
        let buffer = json::Buffer {
            byte_length: buffer_length,
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
            target: Some(Valid(json::buffer::Target::ArrayBuffer)),
        };

        self.buffers.push(buffer);
        self.buffer_views.push(buffer_view);
        let buffer_views_idx = (self.buffer_views.len() - 1) as u32;

        let triangle_acc = json::Accessor {
            buffer_view: Some(json::Index::new(buffer_views_idx)),
            byte_offset: 0,
            count: triangle_len,
            component_type: Valid(json::accessor::GenericComponentType(
                json::accessor::ComponentType::F32,
            )),
            extensions: Default::default(),
            extras: Default::default(),
            type_: Valid(json::accessor::Type::Vec3),
            min: Some(json::Value::from(min)),
            max: Some(json::Value::from(max)),
            name: None,
            normalized: false,
            sparse: None,
        };

        self.accessors.push(triangle_acc);

        let accessors_idx = (self.accessors.len() - 1) as u32;

        self.attr_map.insert(
            Valid(json::mesh::Semantic::Positions),
            json::Index::new(accessors_idx),
        );
        // end single vertex buffer
    }

    fn put_tex(&mut self) {
        let mut buff = BytesMut::with_capacity(1000);
        let input_vec = &self.rdm.vertices;

        for vert in input_vec {
            let t2h = vert.get_t2h();
            buff.put_f32_le(t2h.tex[0].to_f32());
            buff.put_f32_le(t2h.tex[1].to_f32());
        }

        let vec = buff.to_vec(); //stupid
        let tex_len = input_vec.len() as u32;

        let buffer_p = self.obj.push_buffer(vec);
        let buffer_length = buffer_p.len as u32;
        let buffer = json::Buffer {
            byte_length: buffer_length,
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
            target: None,
        };

        self.buffers.push(buffer);
        self.buffer_views.push(buffer_view);
        let buffer_views_idx = (self.buffer_views.len() - 1) as u32;

        let tex_acc = json::Accessor {
            buffer_view: Some(json::Index::new(buffer_views_idx)),
            byte_offset: 0,
            count: tex_len,
            component_type: Valid(json::accessor::GenericComponentType(
                json::accessor::ComponentType::F32,
            )),
            extensions: Default::default(),
            extras: Default::default(),
            type_: Valid(json::accessor::Type::Vec2),
            min: None,
            max: None,
            name: None,
            normalized: false,
            sparse: None,
        };

        self.accessors.push(tex_acc);

        let accessors_idx = (self.accessors.len() - 1) as u32;

        self.attr_map.insert(
            Valid(json::mesh::Semantic::TexCoords(0)),
            json::Index::new(accessors_idx),
        );
    }

    #[allow(dead_code)]
    fn put_normal(&mut self) {
        let mut buff = BytesMut::with_capacity(1000);
        let input_vec = &self.rdm.vertices;

        for vert in input_vec {
            let n4b = vert.get_n4b();

            let nx = ((2.0f32 * n4b.normals[0] as f32) / 255.0f32) - 1.0f32;
            let ny = ((2.0f32 * n4b.normals[1] as f32) / 255.0f32) - 1.0f32;
            let nz = ((2.0f32 * n4b.normals[2] as f32) / 255.0f32) - 1.0f32;

            // calculate unit vector to suppress glTF-Validator ACCESSOR_VECTOR3_NON_UNIT
            //let len = ((nx*nx)+(ny*ny)+(nz*nz)).sqrt();
            //let unx = nx/len;
            //let uny = ny/len;
            //let unz = nz/len;

            buff.put_f32_le(nx);
            buff.put_f32_le(ny);
            buff.put_f32_le(nz);
        }

        let vec = buff.to_vec(); //stupid
        let tex_len = input_vec.len() as u32;

        let buffer_p = self.obj.push_buffer(vec);
        let buffer_length = buffer_p.len as u32;
        let buffer = json::Buffer {
            byte_length: buffer_length,
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
            target: None,
        };

        self.buffers.push(buffer);
        self.buffer_views.push(buffer_view);
        let buffer_views_idx = (self.buffer_views.len() - 1) as u32;

        let normals_acc = json::Accessor {
            buffer_view: Some(json::Index::new(buffer_views_idx)),
            byte_offset: 0,
            count: tex_len,
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

        self.accessors.push(normals_acc);

        let accessors_idx = (self.accessors.len() - 1) as u32;

        self.attr_map.insert(
            Valid(json::mesh::Semantic::Normals),
            json::Index::new(accessors_idx),
        );
    }

    #[allow(dead_code)]
    fn put_tangent(&mut self) {
        let mut buff = BytesMut::with_capacity(1000);
        let input_vec = &self.rdm.vertices;

        for vert in input_vec {
            let g4b = vert.get_g4b();

            let tx = ((2.0f32 * g4b.tangent[0] as f32) / 255.0f32) - 1.0f32;
            let ty = ((2.0f32 * g4b.tangent[1] as f32) / 255.0f32) - 1.0f32;
            let tz = ((2.0f32 * g4b.tangent[2] as f32) / 255.0f32) - 1.0f32;
            let tw_u8 = g4b.tangent[3];

            // calculate unit vector to suppress glTF-Validator ACCESSOR_VECTOR3_NON_UNIT
            // let len = ((tx*tx)+(ty*ty)+(tz*tz)).sqrt();
            // let utx = tx/len;
            // let uty = ty/len;
            // let utz = tz/len;

            buff.put_f32_le(tx);
            buff.put_f32_le(ty);
            buff.put_f32_le(tz);

            if tw_u8 == 1 {
                buff.put_f32_le(1.0);
            } else {
                buff.put_f32_le(-1.0);
            }
        }

        let vec = buff.to_vec(); //stupid
        let tang_len = input_vec.len() as u32;

        let buffer_p = self.obj.push_buffer(vec);
        let buffer_length = buffer_p.len as u32;
        let buffer = json::Buffer {
            byte_length: buffer_length,
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
            target: None,
        };

        self.buffers.push(buffer);
        self.buffer_views.push(buffer_view);
        let buffer_views_idx = (self.buffer_views.len() - 1) as u32;

        let tang_acc = json::Accessor {
            buffer_view: Some(json::Index::new(buffer_views_idx)),
            byte_offset: 0,
            count: tang_len,
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

        self.accessors.push(tang_acc);

        let accessors_idx = (self.accessors.len() - 1) as u32;

        self.attr_map.insert(
            Valid(json::mesh::Semantic::Tangents),
            json::Index::new(accessors_idx),
        );
    }

    fn put_idx(&mut self) {
        // Indexed triangle list
        let triangle_idx: Vec<Triangle> = self.rdm.triangle_indices.clone();
        trace!("triangle_idx.len: {}", triangle_idx.len());

        let triangle_idx_p = self.obj.push_buffer(triangle_idx);
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

        let idx = json::Accessor {
            buffer_view: Some(json::Index::new(buffer_idx_view_idx)),
            byte_offset: 0,
            count: (triangle_idx_p.num * 3),
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
        self.idx = Some(accessors_idx);
        // end Indexed triangle list
    }

    pub fn build(mut self) -> RDGltf {
        let skins = self.get_skins_or_default();
        let animation = if self.anim_node.is_some() {
            vec![self.anim_node.unwrap()]
        } else {
            Default::default()
        };

        let primitive = json::mesh::Primitive {
            attributes: self.attr_map,
            extensions: Default::default(),
            extras: Default::default(),
            indices: Some(json::Index::new(self.idx.unwrap())),
            material: None,
            mode: Valid(json::mesh::Mode::Triangles),
            targets: None,
        };

        let mesh = json::Mesh {
            extensions: Default::default(),
            extras: Default::default(),
            name: None,
            primitives: vec![primitive],
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
            skins,
            animations: animation,
            ..Default::default()
        };

        self.obj.root = Some(root);

        self.obj
    }
}

impl From<RDModell> for RDGltfBuilder {
    fn from(rdm: RDModell) -> Self {
        let has_skin = rdm.has_skin();
        let has_anim = rdm.anim.is_some();

        let mut b = RDGltfBuilder::new(rdm);

        b.put_vertex();
        b.put_idx();

        b.put_tex();
        //b.put_normal();

        //b.put_tangent();

        if has_skin {
            b.put_joint_nodes(JointOption::ResolveParentNode);
            b.put_joint_weight();

            if has_anim {
                b.put_rdm_anim(4 + 1, 5 + 1);
            }
        }

        b
    }
}

pub fn build(rdm: RDModell) {
    let b = RDGltfBuilder::from(rdm);
    let p = b.build();

    p.write_gltf();
}

struct RDGltf {
    buffers: Vec<Vec<u8>>,
    root: Option<json::Root>,
}

impl RDGltf {
    fn new() -> Self {
        RDGltf {
            buffers: vec![],
            root: None,
        }
    }

    fn write_gltf(mut self) {
        let _ = fs::create_dir("gltf_out");

        let writer = fs::File::create("gltf_out/out.gltf").expect("I/O error");
        json::serialize::to_writer_pretty(writer, &self.root.unwrap())
            .expect("Serialization error");

        let mut idx = self.buffers.len() - 1;
        while !self.buffers.is_empty() {
            let e = self.buffers.pop().unwrap();
            let bin = e;
            let file_path = format!("gltf_out/buffer{}.bin", idx);
            let mut writer = fs::File::create(file_path).expect("I/O error");
            writer.write_all(&bin).expect("I/O error");

            idx = idx.saturating_sub(1);
        }
    }

    fn push_buffer<T>(&mut self, vec: Vec<T>) -> PushBufferResult {
        let idx = self.buffers.len();

        let file_name = format!("buffer{}.bin", idx);
        let num = vec.len();

        let padded_byte_vector = to_padded_byte_vector(vec);
        let len = padded_byte_vector.len();
        self.buffers.push(padded_byte_vector);

        PushBufferResult {
            file_name,
            num: num as u32,
            len: len as u32,
            idx: idx as u32,
        }
    }
}

struct PushBufferResult {
    file_name: String,
    num: u32,
    len: u32,
    idx: u32,
}
