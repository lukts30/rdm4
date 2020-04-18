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

#[derive(Copy, Clone, Debug)]
#[repr(C)]
struct Vertex {
    position: [f32; 3],
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
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

pub fn rdm_joint_weights(input_vec: &Vec<VertexFormat>) -> (
    json::Buffer,
    json::buffer::View,
    json::Accessor,
    json::buffer::View,
    json::Accessor,
) {
    let mut joint_weight_buf = BytesMut::with_capacity((4 + 4 * 4) * input_vec.len());
    let def_weight: [f32; 4] = [1.0, 0.0, 0.0, 0.0];

    for vert in input_vec {
        match vert {
            VertexFormat::P4h_N4b_G4b_B4b_T2h_I4b(_, _, _, _, _, i4b) => {
                joint_weight_buf.put_slice(&i4b.blend_idx);
                joint_weight_buf.put_f32_le(def_weight[0]);
                joint_weight_buf.put_f32_le(def_weight[1]);
                joint_weight_buf.put_f32_le(def_weight[2]);
                joint_weight_buf.put_f32_le(def_weight[3]);
            }
            _ => panic!("not supported !"),
        };
    }

    let real_len = joint_weight_buf.len() as u32;
    joint_weight_buf.put_u32_le(0);

    let mut writer = fs::File::create("triangle/buffer3.bin").expect("I/O error");
    writer.write_all(&joint_weight_buf).expect("I/O error");



    let jw_buffer = json::Buffer {
        byte_length: joint_weight_buf.len() as u32,
        extensions: Default::default(),
        extras: Default::default(),
        name: None,
        uri: Some("buffer3.bin".into()),
    };

    let joint_buffer_view = json::buffer::View {
        buffer: json::Index::new(3),
        byte_length: real_len,
        byte_offset: None,
        byte_stride: Some(20),
        extensions: Default::default(),
        extras: Default::default(),
        name: None,
        target: None,
    };

    let joint_accessor = json::Accessor {
        buffer_view: Some(json::Index::new(3)),
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

    let weight_buffer_view = json::buffer::View {
        buffer: json::Index::new(3),
        byte_length: real_len,
        byte_offset: Some(4),
        byte_stride: Some(20),
        extensions: Default::default(),
        extras: Default::default(),
        name: None,
        target: None,
    };

    let weight_accessor = json::Accessor {
        buffer_view: Some(json::Index::new(4)),
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

    (jw_buffer,joint_buffer_view,joint_accessor,weight_buffer_view,weight_accessor)

}

pub fn rdm_joint_to_nodes(
    cfg: JointOption,
    mut joints_vec: Vec<RDJoint>,
    start_jindex: u32,
) -> (
    u32,
    Vec<json::Node>,
    json::Buffer,
    json::buffer::View,
    json::Accessor,
) {
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

            let bindmat = (ct.to_homogeneous()) * (uq.to_homogeneous()) * Matrix4::identity();
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
            invbind_buf.put_f32_le(inv_bindmat.m44);

            let invbind_buf_written = invbind_buf.len() - invbind_buf_len;
            assert_eq!(invbind_buf_written, 64);
        }

        //let bin = to_padded_byte_vector(invbind_buf);
        let mut writer = fs::File::create("triangle/buffer2.bin").expect("I/O error");
        writer.write_all(&invbind_buf).expect("I/O error");
    }

    let mat_buffer = json::Buffer {
        byte_length: invbind_buf.len() as u32,
        extensions: Default::default(),
        extras: Default::default(),
        name: None,
        uri: Some("buffer2.bin".into()),
    };

    let mat_buffer_view = json::buffer::View {
        buffer: json::Index::new(2),
        byte_length: invbind_buf.len() as u32,
        byte_offset: None,
        byte_stride: None,
        extensions: Default::default(),
        extras: Default::default(),
        name: None,
        target: None,
    };

    let mat_accessor = json::Accessor {
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

    for i in 0..joints_vec.len() {
        if joints_vec[i].parent == 255 || cfg == JointOption::ResolveAllRoot {
            joints_vec[i].locked = true;
            arm.push(json::Index::new(start_jindex + i as u32));
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
            if joints_vec[j].parent == z as u8
                && joints_vec[z].locked == true
                && joints_vec[j].locked == false
            {
                joints_vec[j].locked = true;
                child.push(gltf_json::Index::new(start_jindex + j as u32));
                tb_rel.push_back((z, j));
            }
        }

        if !child.is_empty() && cfg == JointOption::ResolveParentNode {
            child_list.push_back(Some(child))
        } else {
            child_list.push_back(None);
        }
    }

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
            children: {
                let p = child_list.pop_front().unwrap();
                p
            },
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
    (
        skin_nodes.len() as u32,
        skin_nodes,
        mat_buffer,
        mat_buffer_view,
        mat_accessor,
    )
}

fn rdm_vertex_to_gltf(input_vec: Vec<VertexFormat>) -> (Vec<Vertex>, Vec<f32>, Vec<f32>) {
    let mut out: Vec<Vertex> = Vec::new();

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

pub fn export(rdm: RDModell) {
    // skinning joints and weights
    let jw = rdm_joint_weights(&rdm.vertices);
    //

    let conv = rdm_vertex_to_gltf(rdm.vertices);

    let triangle_vertices = conv.0;
    let min = conv.1;
    let max = conv.2;

    let triangle_idx: Vec<Triangle> = rdm.triangle_indices;

    let buffer_length = (triangle_vertices.len() * mem::size_of::<Vertex>()) as u32;
    let buffer = json::Buffer {
        byte_length: buffer_length,
        extensions: Default::default(),
        extras: Default::default(),
        name: None,
        uri: Some("buffer0.bin".into()),
    };
    let buffer_view = json::buffer::View {
        buffer: json::Index::new(0),
        byte_length: buffer.byte_length,
        byte_offset: None,
        byte_stride: None,
        extensions: Default::default(),
        extras: Default::default(),
        name: None,
        target: Some(Valid(json::buffer::Target::ArrayBuffer)),
    };
    let positions = json::Accessor {
        buffer_view: Some(json::Index::new(0)),
        byte_offset: 0,
        count: triangle_vertices.len() as u32,
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

    let triangle_idx_len_b = (triangle_idx.len() * mem::size_of::<Triangle>()) as u32;

    let buffer_idx = json::Buffer {
        byte_length: triangle_idx_len_b,
        extensions: Default::default(),
        extras: Default::default(),
        name: None,
        uri: Some("buffer1.bin".into()),
    };
    let buffer_idx_view = json::buffer::View {
        buffer: json::Index::new(1),
        byte_length: triangle_idx_len_b,
        byte_offset: None,
        byte_stride: None,
        extensions: Default::default(),
        extras: Default::default(),
        name: None,
        target: Some(Valid(json::buffer::Target::ElementArrayBuffer)),
    };

    let idx = json::Accessor {
        buffer_view: Some(json::Index::new(1)),
        byte_offset: 0,
        count: (triangle_idx.len() * 3) as u32,
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

    let primitive = json::mesh::Primitive {
        attributes: {
            let mut map = std::collections::HashMap::new();
            map.insert(Valid(json::mesh::Semantic::Positions), json::Index::new(0));
            map.insert(Valid(json::mesh::Semantic::Joints(0)), json::Index::new(3));
            map.insert(Valid(json::mesh::Semantic::Weights(0)), json::Index::new(4));
            map
        },
        extensions: Default::default(),
        extras: Default::default(),
        indices: Some(json::Index::new(1)),
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

    let mut nlen = json::Index::new(0);
    let mut njvec: Option<Vec<json::Node>> = None;

    let mut vec_acc = vec![positions, idx];
    let mut vec_buff = vec![buffer, buffer_idx];
    let mut vec_buff_v = vec![buffer_view, buffer_idx_view];

    let mut sk = None;

    if rdm.joints.is_some() {
        let comb = rdm_joint_to_nodes(JointOption::ResolveParentNode, rdm.joints.unwrap(), 0);

        let nlen_u32 = comb.0-1;
        nlen = json::Index::new(nlen_u32);
        njvec = Some(comb.1);

        let mut joint_indi_vec: Vec<json::root::Index<_>> = Vec::new();
        for i in 0..nlen_u32 {
            joint_indi_vec.push(json::Index::new(i as u32));
        }

        sk = Some(json::Skin {
            joints: joint_indi_vec,
            extensions: None,
            inverse_bind_matrices: Some(json::Index::new(2)),
            skeleton: None,
            extras: None,
            name: None,
        });

        //
        let mat_buff = comb.2;
        vec_buff.push(mat_buff);
        
        let jw_buff = jw.0;
        vec_buff.push(jw_buff);
        //

        //
        let mat_buff_v = comb.3;
        vec_buff_v.push(mat_buff_v);

        let joint_buff_v = jw.1;
        let weight_buff_v = jw.3;
        vec_buff_v.push(joint_buff_v);
        vec_buff_v.push(weight_buff_v);
        //

        //
        let mat_acc = comb.4;
        vec_acc.push(mat_acc);

        let joint_acc = jw.2;
        let weight_acc = jw.4;
        vec_acc.push(joint_acc);
        vec_acc.push(weight_acc);
        //
    }


    warn!("nlen: {}", nlen);

    let node = json::Node {
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

    let root = json::Root {
        accessors: vec_acc,
        buffers: vec_buff,
        buffer_views: vec_buff_v,
        meshes: vec![mesh],
        nodes: njvec.unwrap_or_else(|| vec![node]),
        scenes: vec![json::Scene {
            extensions: Default::default(),
            extras: Default::default(),
            name: None,
            nodes: vec![nlen],
        }],
        skins: vec![sk.unwrap()],
        ..Default::default()
    };

    let _ = fs::create_dir("triangle");

    let writer = fs::File::create("triangle/triangle.gltf").expect("I/O error");
    json::serialize::to_writer_pretty(writer, &root).expect("Serialization error");

    let bin = to_padded_byte_vector(triangle_vertices);
    let mut writer = fs::File::create("triangle/buffer0.bin").expect("I/O error");
    writer.write_all(&bin).expect("I/O error");

    let bin2 = to_padded_byte_vector(triangle_idx);
    let mut writer2 = fs::File::create("triangle/buffer1.bin").expect("I/O error");
    writer2.write_all(&bin2).expect("I/O error");
}
