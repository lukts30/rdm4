use gltf;

use crate::RDJoint;
use crate::RDModell;
use crate::Triangle;
use crate::VertexFormat;

use crate::B4b;
use crate::G4b;
use crate::I4b;
use crate::N4b;
use crate::P4h;
use crate::T2h;
use nalgebra::*;

use half::f16;

use bytes::Bytes;

use crate::rdm_anim::*;
use gltf::animation::util::ReadOutputs::*;

use nalgebra::*;

#[test]
fn find_parent() {
    let (gltf, buffers, _) = gltf::import("triangle/triangle.gltf").unwrap();
    for scene in gltf.scenes() {}
}

#[test]
fn root() {
    let (gltf, buffers, _) = gltf::import("triangle/triangle.gltf").unwrap();
    for scene in gltf.scenes() {
        for node in scene.nodes() {
            println!(
                "Node #{} has {} children",
                node.index(),
                node.children().count(),
            );
            trav(node.children());
        }
    }
}

fn trav<'a>(a: gltf::scene::iter::Children<'a>) {
    for node in a {
        if node.skin().is_some() {
            println!("..");
            break;
        }
        println!("#{} has {} children", node.index(), node.children().count(),);

        for n in node.children() {
            if n.index() == 3 {
                panic!("--");
            }
        }
        trav(node.children());
    }
}

pub fn anim() -> Option<RDAnim> {
    let (gltf, buffers, _) = gltf::import("triangle/triangle.gltf").unwrap();

    let mut anim = None;

    for animation in gltf.animations() {
        println!("animations #{}", animation.name().unwrap());

        let mut anim_vec: Vec<FrameCollection> = Vec::new();

        let mut frames_rot: Vec<Frame> = Vec::new();
        let mut frames_trans: Vec<Frame> = Vec::new();

        let mut t_max = 0.0;

        for (j, channel) in animation.channels().enumerate() {
            let reader = channel.reader(|buffer| Some(&buffers[buffer.index()]));
            let mut time = reader.read_inputs().unwrap();
            let mut output = reader.read_outputs().unwrap();

            println!("{}", time.len());
            println!(
                "channel #{} |  {:?} ",
                channel.target().node().name().unwrap(),
                channel.target().property()
            );

            // ugly as hell
            t_max = t_max.max(
                channel
                    .sampler()
                    .input()
                    .max()
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .get(0)
                    .unwrap()
                    .as_f64()
                    .unwrap(),
            );

            match output {
                Rotations(rot) => {
                    let mut rot_iter = rot.into_f32();
                    for t in time {
                        let r = rot_iter.next().unwrap();
                        let f = Frame {
                            time: t,
                            rotation: [r[0], r[1], r[2], -r[3]],
                            translation: [0.0, 0.0, 0.0],
                        };
                        frames_rot.push(f);
                    }
                }
                Translations(mut trans) => {
                    for t in time {
                        let f = Frame {
                            time: t,
                            rotation: [0.0, 0.0, 0.0, 0.0],
                            translation: trans.next().unwrap(),
                        };
                        frames_trans.push(f);
                    }
                }
                _ => {
                    println!("not supported !");
                    continue;
                }
            };

            // merge rot and trans vecs. assumse one rotation and trans successive

            if j % 2 == 1 {
                for (j, f) in frames_rot.iter_mut().enumerate() {
                    f.translation = frames_trans[j].translation;
                }
                let c = FrameCollection {
                    name: String::from(channel.target().node().name().unwrap()),
                    len: frames_rot.len() as u32,
                    frames: frames_rot,
                };
                anim_vec.push(c);
                frames_rot = Vec::new();
                frames_trans.clear();
            }

            /*
            if frames_rot.len()>0 {
                for (k, f) in frames_trans.iter_mut().enumerate() {
                    f.rotation = frames_rot[k].rotation;
                }
                let c = FrameCollection {
                    name: String::from(channel.target().node().name().unwrap()),
                    len: frames_rot.len() as u32,
                    frames: frames_trans,
                };
                anim_vec.push(c);

                frames_trans = Vec::new();
                frames_rot.clear();

                println!("pushed: {}",channel.target().node().name().unwrap());
            }
            */
        }

        anim = Some(RDAnim {
            time_max: (t_max * 1000.0) as u32,
            anim_vec,
            name: String::from(animation.name().unwrap()),
        });

        /*
        println!("time_max : {}", anim.time_max);
        for a in anim.anim_vec.iter() {
            println!(" : {}", a.name);
        }
        */
    }
    anim
}

pub fn conv() -> RDModell {
    let gltf_imp = start().unwrap();
    let size = 0;
    let vertices_vec = gltf_imp.0;
    let triangles = gltf_imp.1;

    let meta = 0;
    let vertex_offset = 0;

    let vertices_count = vertices_vec.len() as u32;
    let vertex_buffer_size = 28;

    let triangles_offset = 0;

    let triangles_idx_count = triangles.len() as u32 * 3;
    let triangles_idx_size = 2;

    let joints_vec = skin();

    let rd = RDModell {
        size,
        buffer: Bytes::new(),
        joints: Some(joints_vec),
        vertices: vertices_vec,
        triangle_indices: triangles,
        meta,
        vertex_offset,
        vertices_count,
        vertex_buffer_size,

        triangles_offset,
        triangles_idx_count,
        triangles_idx_size,

        anim: None,
    };

    rd
}

fn skin() -> Vec<RDJoint> {
    let mut out_joints_vec = Vec::new();

    let mut node_names_vec = Vec::new();

    let (gltf, buffers, _) = gltf::import("triangle/triangle.gltf").unwrap();
    for skin in gltf.skins() {
        println!("skin #{}", skin.index());

        for node in skin.joints().into_iter() {
            node_names_vec.push(node.name().unwrap());
        }

        println!("{:?}", node_names_vec);
        //let mut search_vec = node_names_vec.into_iter().cycle();
        let mut node_vec: Vec<u8> = vec![255; skin.joints().count()];

        for (i, node) in skin.joints().into_iter().enumerate() {
            let master_name = node.name().unwrap();
            //let parent = search_vec.position(|r: &str| r == master_name).unwrap();
            //println!("master_name[{}]: {} ",parent,master_name);

            //println!("master_name: {}",master_name);
            for child in node.children() {
                println!("{}: {}", child.name().unwrap(), i);

                let child_idx = node_names_vec
                    .iter()
                    .position(|&r| r == child.name().unwrap())
                    .unwrap();
                node_vec[child_idx] = i as u8;
            }
        }

        println!("{:?}", node_vec);
        let mut node_vec_iter = node_vec.iter();
        let mut node_names_vec_iter = node_names_vec.iter();

        let reader = skin.reader(|buffer| Some(&buffers[buffer.index()]));

        let mut mats_iter = reader.read_inverse_bind_matrices().unwrap();
        let mut count = mats_iter.len();

        while count > 0 {
            let mut mat4: Matrix4<f32> = Matrix4::identity();

            let mat = mats_iter.next().unwrap();

            mat4.m11 = mat[0][0];
            mat4.m21 = mat[0][1];
            mat4.m31 = mat[0][2];
            mat4.m41 = mat[0][3];

            mat4.m12 = mat[1][0];
            mat4.m22 = mat[1][1];
            mat4.m32 = mat[1][2];
            mat4.m42 = mat[1][3];

            mat4.m13 = mat[2][0];
            mat4.m23 = mat[2][1];
            mat4.m33 = mat[2][2];
            mat4.m43 = mat[2][3];

            mat4.m14 = mat[3][0];
            mat4.m24 = mat[3][1];
            mat4.m34 = mat[3][2];
            mat4.m44 = mat[3][3];

            let mat3 = Matrix3::new(
                mat4.m11, mat4.m12, mat4.m13, mat4.m21, mat4.m22, mat4.m23, mat4.m31, mat4.m32,
                mat4.m33,
            );
            let rot = Rotation3::from_matrix(&mat3);
            let q = UnitQuaternion::from_rotation_matrix(&rot).inverse().coords;


            let qq = Quaternion::new(q.w, q.x, q.y, q.z);
            let mut uq = UnitQuaternion::from_quaternion(qq);

            

            println!("pq {:?}",uq);

            
            let tx = mat4.m14;
            let ty = mat4.m24;
            let tz = mat4.m34;
            
            let joint_translatio: Translation3<f32> = Translation3::new(tx, ty, tz);

            let inv_bindmat = (uq.to_homogeneous()) * (joint_translatio.to_homogeneous());
            let iv_x = inv_bindmat.m14;
            let iv_y = inv_bindmat.m24;
            let iv_z = inv_bindmat.m34;

            let trans_point = Translation3::new(iv_x, iv_y, iv_z).inverse();

            println!("trans : {:#?}", trans_point);
            

            
            let quaternion_mat4 = uq.quaternion().coords;

            let rdjoint = RDJoint {
                nameptr: 0,
                name: String::from(*node_names_vec_iter.next().unwrap()),
                locked: false,
                parent: *node_vec_iter.next().unwrap(),
                quaternion: [
                    quaternion_mat4.x,
                    quaternion_mat4.y,
                    quaternion_mat4.z,
                    quaternion_mat4.w,
                ],
                transition: [trans_point.x, trans_point.y, trans_point.z],
            };
            out_joints_vec.push(rdjoint);

            count = count - 1;
        }
    }
    out_joints_vec
}

fn start() -> Option<(Vec<VertexFormat>, Vec<Triangle>)> {
    let (gltf, buffers, _) = gltf::import("triangle/triangle.gltf").unwrap();
    for mesh in gltf.meshes() {
        println!("Mesh #{}", mesh.index());
        for primitive in mesh.primitives() {
            println!("- Primitive #{}", primitive.index());
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

            let mut position_iter = reader.read_positions().unwrap();

            //let mut normal_iter = reader.read_normals().unwrap();
            //let mut tangent_iter = reader.read_tangents().unwrap();

            // let mut tex_iter = reader.read_tex_coords(0).unwrap().into_f32();

            let mut joints_iter = reader.read_joints(0).unwrap().into_u16();

            let mut count = position_iter.len();

            let mut verts_vec: Vec<VertexFormat> = Vec::with_capacity(count);

            while count > 0 {
                let vertex_position = position_iter.next().unwrap();
                let p4h = P4h {
                    pos: [
                        f16::from_f32(vertex_position[0]),
                        f16::from_f32(vertex_position[1]),
                        f16::from_f32(vertex_position[2]),
                        f16::from_f32(0.0),
                    ],
                };

                //let normals = normal_iter.next().unwrap();
                let normals = [0.0,0.0,0.0,0.0];
                let nx = normals[0];
                let ny = normals[1];
                let nz = normals[2];

                let n4b = N4b {
                    normals: [
                        (nx * (255.0 / 2.0) + 255.0 / 2.0) as u8,
                        (ny * (255.0 / 2.0) + 255.0 / 2.0) as u8,
                        (nz * (255.0 / 2.0) + 255.0 / 2.0) as u8,
                        0,
                    ],
                };

                //let tangents = tangent_iter.next().unwrap();
                let tx = 0.25;
                let ty = 0.25;
                let tz = 0.5;
                let tw = -1.0;

                let g4b = G4b {
                    tangent: [
                        (tx * (255.0 / 2.0) + 255.0 / 2.0) as u8,
                        (ty * (255.0 / 2.0) + 255.0 / 2.0) as u8,
                        (tz * (255.0 / 2.0) + 255.0 / 2.0) as u8,
                        {
                            let is_neg = relative_eq!(tw, -1.0);
                            if is_neg {
                                0
                            } else {
                                1
                            }
                        },
                    ],
                };

                // bi

                let normal = Vector3::new(nx, ny, nz);
                let tangent = Vector3::new(tx, ty, tz);

                let b: Matrix3x1<f32> = (normal.cross(&tangent)) * (tw);

                //println!("bbbbb: {:?}",b);

                let b4b = B4b {
                    binormal: [
                        ((b.x * (255.0 / 2.0) + 255.0 / 2.0) as u8).saturating_add(1),
                        ((b.y * (255.0 / 2.0) + 255.0 / 2.0) as u8).saturating_add(1),
                        ((b.z * (255.0 / 2.0) + 255.0 / 2.0) as u8).saturating_add(1),
                        0,
                    ],
                };

                // tex

                //let tex = tex_iter.next().unwrap();
                let tex = [0.0, 0.0];
                let t2h = T2h {
                    tex: [f16::from_f32(tex[0]), f16::from_f32(tex[1])],
                };

                // joints idx

                let joint = joints_iter.next().unwrap();

                let i4b = I4b {
                    blend_idx: [
                        joint[0] as u8,
                        joint[1] as u8,
                        joint[2] as u8,
                        joint[3] as u8,
                    ],
                };

                let k = VertexFormat::P4h_N4b_G4b_B4b_T2h_I4b(p4h, n4b, g4b, b4b, t2h, i4b);
                //let k = VertexFormat::P4h_N4b_T2h_I4b(p4h, n4b,t2h, i4b);
                verts_vec.push(k);
                count = count - 1;
            }

            println!("verts_vec {}", verts_vec.len());

            let mut triangle_iter = reader.read_indices().unwrap().into_u32();
            let mut triangle_vec: Vec<Triangle> = Vec::with_capacity(count);

            let mut tcount = triangle_iter.len() / 3;

            while tcount > 0 {
                //let ctri = triangle_iter.next().unwrap();
                let t = Triangle {
                    indices: [
                        triangle_iter.next().unwrap() as u16,
                        triangle_iter.next().unwrap() as u16,
                        triangle_iter.next().unwrap() as u16,
                    ],
                };
                tcount = tcount - 1;
                triangle_vec.push(t);
            }

            return Some((verts_vec, triangle_vec));
        }
    }
    None
}

#[test]
fn rot_tran() {
    
    let (gltf, buffers, _) = gltf::import("triangle/triangle.gltf").unwrap();
    for skin in gltf.skins() {
        println!("skin #{}", skin.index());

        let reader = skin.reader(|buffer| Some(&buffers[buffer.index()]));

        let mut mats_iter = reader.read_inverse_bind_matrices().unwrap();

        for j in skin.joints().into_iter() {

            println!("Node: {} {:?}", j.name().unwrap(),j.transform());

            let mut mat4: Matrix4<f32> = Matrix4::identity();
            let mat = mats_iter.next().unwrap();

            mat4.m11 = mat[0][0];
            mat4.m21 = mat[0][1];
            mat4.m31 = mat[0][2];
            mat4.m41 = mat[0][3];

            mat4.m12 = mat[1][0];
            mat4.m22 = mat[1][1];
            mat4.m32 = mat[1][2];
            mat4.m42 = mat[1][3];

            mat4.m13 = mat[2][0];
            mat4.m23 = mat[2][1];
            mat4.m33 = mat[2][2];
            mat4.m43 = mat[2][3];

            mat4.m14 = mat[3][0];
            mat4.m24 = mat[3][1];
            mat4.m34 = mat[3][2];
            mat4.m44 = mat[3][3];


            // Node Mat T * R * S
            let node_transform_mat4 = j.transform().matrix();

            let mut node_mat4: Matrix4<f32> = Matrix4::identity();
            
            node_mat4.m11 = node_transform_mat4[0][0];
            node_mat4.m21 = node_transform_mat4[0][1];
            node_mat4.m31 = node_transform_mat4[0][2];
            node_mat4.m41 = node_transform_mat4[0][3];

            node_mat4.m12 = node_transform_mat4[1][0];
            node_mat4.m22 = node_transform_mat4[1][1];
            node_mat4.m32 = node_transform_mat4[1][2];
            node_mat4.m42 = node_transform_mat4[1][3];

            node_mat4.m13 = node_transform_mat4[2][0];
            node_mat4.m23 = node_transform_mat4[2][1];
            node_mat4.m33 = node_transform_mat4[2][2];
            node_mat4.m43 = node_transform_mat4[2][3];

            node_mat4.m14 = node_transform_mat4[3][0];
            node_mat4.m24 = node_transform_mat4[3][1];
            node_mat4.m34 = node_transform_mat4[3][2];
            node_mat4.m44 = node_transform_mat4[3][3];


            let mat3 = Matrix3::new(
                mat4.m11, mat4.m12, mat4.m13, mat4.m21, mat4.m22, mat4.m23, mat4.m31, mat4.m32,
                mat4.m33,
            );
            let rot = Rotation3::from_matrix(&mat3);
            let uq:UnitQuaternion<f32> = UnitQuaternion::from(rot);

            println!("rot: {:?}",uq);
            // 
            let r_mat4 = mat4*node_mat4;

            println!("r_mat4: {:#?}",r_mat4);
            
        }
    
    }

}