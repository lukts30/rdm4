use crate::vertex::*;
use crate::{rdm_writer::PutVertex, RdJoint};
use crate::{vertex::TargetVertexFormat, Triangle};
use crate::{MeshInstance, RdModell};

use gltf::Node;
use nalgebra::*;

use half::f16;

use bytes::{Bytes, BytesMut};

use crate::rdm_anim::*;
use gltf::animation::util::ReadOutputs::*;

use crate::VertexFormat2;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::u16;
use std::{
    collections::HashMap,
    convert::{TryFrom, TryInto},
};

fn node_get_local_transform(target_node: &Node) -> Isometry3<f32> {
    let target_mat = target_node.transform().matrix();
    let mut mat = Matrix4::from_fn_generic(U4, U4, |i, j| target_mat[j][i]);
    mat.m44 = 1.0;
    let similarity: Similarity3<f32> = nalgebra::try_convert(mat).unwrap();
    debug!("similarity.scaling: {}", similarity.scaling());

    let mut isometry: Isometry3<f32> = similarity.isometry;
    isometry.rotation = isometry.rotation.inverse();
    isometry
}

fn node_get_name(target_node: &Node) -> String {
    match target_node.name() {
        Some(name) => name.to_owned(),
        None => format!("UnnamedGltfNode{}", target_node.index()),
    }
}

pub fn read_animation(
    f_path: &Path,
    joints: &[RdJoint],
    frames: usize,
    tmax: f32,
) -> Option<Vec<RdAnim>> {
    let (gltf, buffers, _) = gltf::import(f_path).unwrap();
    let mut translation_map: HashMap<String, Vec<Frame>> = HashMap::new();
    let mut rd_animations = Vec::new();

    let mut real_joints: Vec<_> = joints.iter().map(|e| e.name.as_str()).collect();
    real_joints.sort_unstable();

    let interpolate_error_message = "Interpolate required but not supported ! Re-Export model in blender with 'Always sample animations' enabled and try again";

    for (anim_idx, animation) in gltf.animations().enumerate() {
        let mut t_max = 0.0;
        debug!("animation: {}", animation.name().unwrap_or("default"));
        for (_, channel) in animation.channels().enumerate() {
            let reader = channel.reader(|buffer| Some(&buffers[buffer.index()]));
            let time = reader.read_inputs().unwrap();
            let output = reader.read_outputs().unwrap();

            let target_node_name_v2 = node_get_name(&channel.target().node());

            debug!("{}", time.len());
            info!(
                "channel: {} | {:?} ",
                target_node_name_v2,
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

            let origin = node_get_local_transform(&channel.target().node());
            let origin_translation = [
                origin.translation.x,
                origin.translation.y,
                origin.translation.z,
            ];
            let origin_quaternio_raw = origin.rotation.quaternion().coords;
            let origin_rotation = [
                origin_quaternio_raw.x,
                origin_quaternio_raw.y,
                origin_quaternio_raw.z,
                origin_quaternio_raw.w,
            ];

            if real_joints.iter().any(|&x| x == target_node_name_v2) {
                match output {
                    Rotations(rot) => {
                        let mut rot_iter = rot.into_f32();
                        if let Some(frames) = translation_map.get_mut(&target_node_name_v2) {
                            for (frame, t) in frames.iter_mut().zip(time) {
                                assert!(
                                    relative_eq!(t, frame.time),
                                    "{}",
                                    interpolate_error_message
                                );
                                let r = rot_iter.next().unwrap();
                                frame.rotation = [r[0], r[1], r[2], -r[3]];
                            }
                        } else {
                            let mut frames_rot: Vec<Frame> = Vec::new();
                            for t in time {
                                let r = rot_iter.next().unwrap();
                                let f = Frame {
                                    time: t,
                                    rotation: [r[0], r[1], r[2], -r[3]],
                                    translation: origin_translation,
                                };
                                frames_rot.push(f);
                            }
                            translation_map.insert(target_node_name_v2, frames_rot);
                        }
                    }
                    Translations(mut trans) => {
                        if let Some(frames) = translation_map.get_mut(&target_node_name_v2) {
                            for (frame, t) in frames.iter_mut().zip(time) {
                                assert!(
                                    relative_eq!(t, frame.time),
                                    "{}",
                                    interpolate_error_message
                                );
                                frame.translation = trans.next().unwrap();
                            }
                        } else {
                            let mut frames: Vec<Frame> = Vec::new();
                            for t in time {
                                let f = Frame {
                                    time: t,
                                    rotation: origin_rotation,
                                    translation: trans.next().unwrap(),
                                };
                                frames.push(f);
                            }
                            translation_map.insert(target_node_name_v2, frames);
                        };
                    }
                    _ => {
                        warn!(
                            "output sampler not supported: '{:?}'",
                            channel.target().property()
                        );
                        continue;
                    }
                }
            } else {
                error!("Node: {:?} is referenced by an animation channel but the node is not part of the skinned mesh inverseBindMatrices!",target_node_name_v2)
            }
        }

        for joint in joints {
            if !translation_map.contains_key(&joint.name) {
                let node_idx = gltf
                    .nodes()
                    .find(|n| node_get_name(n) == joint.name)
                    .unwrap();
                let origin = node_get_local_transform(&node_idx);
                let origin_translation = [
                    origin.translation.x,
                    origin.translation.y,
                    origin.translation.z,
                ];
                let origin_quaternio_raw = origin.rotation.quaternion().coords;
                let origin_rotation = [
                    origin_quaternio_raw.x,
                    origin_quaternio_raw.y,
                    origin_quaternio_raw.z,
                    origin_quaternio_raw.w,
                ];
                let intervall = tmax / (frames as f32 - 1.0);
                let mut v = Vec::with_capacity(frames);
                for i in 0..frames {
                    v.push(Frame {
                        rotation: origin_rotation,
                        translation: origin_translation,
                        time: i as f32 * intervall,
                    });
                }
                warn!("idle_anim: adding idle for joint: {:?}", joint.name);
                translation_map.insert(joint.name.clone(), v);
            }
        }

        let mut frame_collections: Vec<FrameCollection> = Vec::new();
        for (node_str, frames) in translation_map.drain() {
            frame_collections.push(FrameCollection {
                len: frames.len() as u32,
                frames,
                name: node_str.to_string(),
            })
        }

        assert_eq!(joints.len() - frame_collections.len(), 0);

        let name = format!("anim_{}", anim_idx);
        rd_animations.push(RdAnim {
            time_max: (t_max * 1000.0) as u32,
            anim_vec: frame_collections,
            name,
        });
    }
    Some(rd_animations)
}

pub fn load_gltf(
    f_path: &Path,
    dst_format: TargetVertexFormat,
    load_skin: bool,
    negative_x_and_v0v2v1: bool,
    no_transform: bool,
    overide_mesh_idx: Option<Vec<u32>>,
) -> RdModell {
    info!("gltf::import start!");
    let (gltf, buffers, _) = gltf::import(f_path).unwrap();
    info!("gltf::import end!");
    if negative_x_and_v0v2v1 {
        warn!("negative_x_and_v0v2v1: {}", negative_x_and_v0v2v1);
        warn!("negative_x_and_v0v2v1 may cause lighting artifacts !");
    }
    let gltf_imp = read_mesh(
        &gltf,
        &buffers,
        dst_format,
        load_skin,
        negative_x_and_v0v2v1,
        no_transform,
        overide_mesh_idx,
    )
    .unwrap();
    let size = 0;
    let vertices = gltf_imp.1;
    let triangles = gltf_imp.2;

    let meta = 0;

    let triangles_offset = 0;

    let triangles_idx_count = triangles.len() as u32 * 3;
    let triangles_idx_size = 2;

    let joints_vec = if load_skin {
        Some(read_skin(&gltf, &buffers))
    } else {
        None
    };

    // todo!("TODO : FIX ME !!!");
    let mesh_info_vec = gltf_imp.4;
    RdModell {
        size,
        buffer: Bytes::new(),
        mesh_info: mesh_info_vec,
        joints: joints_vec,
        triangle_indices: triangles,
        meta,
        vertex: vertices,

        triangles_offset,
        triangles_idx_count,
        triangles_idx_size,

        anim: None,
        mat: None,
    }
}

fn read_skin(gltf: &gltf::Document, buffers: &[gltf::buffer::Data]) -> Vec<RdJoint> {
    let mut out_joints_vec = Vec::new();

    for skin in gltf.skins() {
        let mut node_names_vec: Vec<String> = Vec::new();

        info!("skin #{}", skin.index());

        for node in skin.joints() {
            node_names_vec.push(node_get_name(&node));
        }

        debug!("{:?}", node_names_vec);
        // parentless nodes have 255 as "index"
        let mut node_vec: Vec<u8> = vec![255; skin.joints().count()];

        for (i, node) in skin.joints().enumerate() {
            for child in node.children() {
                //rdm: children know their parent VS glTF parents know their children
                let c_name = node_get_name(&child);

                let child_idx = node_names_vec.iter().position(|r| r == &c_name).unwrap();
                node_vec[child_idx] = i as u8;
                debug!("{}: {} -> {}", c_name, i, node_names_vec[i]);
            }
        }

        debug!("node_vec: {:?}", node_vec);
        let node_vec_iter = node_vec.into_iter();
        let node_names_vec_iter = node_names_vec.into_iter();

        let reader = skin.reader(|buffer| Some(&buffers[buffer.index()]));

        let mats_iter = reader.read_inverse_bind_matrices().unwrap();
        for (z, ((mat, parent), name)) in mats_iter
            .zip(node_vec_iter)
            .zip(node_names_vec_iter)
            .enumerate()
        {
            let inverse_bind_matrix: Matrix4<f32> =
                Matrix4::from_fn_generic(U4, U4, |i, j| mat[j][i]);
            // inverseBindMatrix^-1 = BindMatrix
            // BindMatrix: global transform of the respective joint
            let mut mat4_init: Matrix4<f32> = inverse_bind_matrix.try_inverse().unwrap();

            // may perform expensive checks ...
            debug!("{} mat4_init: {}", z, mat4_init);
            mat4_init.m44 = 1.0;
            let similarity: Similarity3<f32> = nalgebra::try_convert(mat4_init).unwrap();
            debug!("similarity.scaling: {}", similarity.scaling());

            let isometry: Isometry3<f32> = similarity.isometry;
            let unit_quaternion = isometry.rotation;
            let quaternion_raw = unit_quaternion.quaternion().coords;

            let translation: Translation3<f32> = isometry.translation;

            let rdjoint = RdJoint {
                nameptr: 0,
                name,
                locked: false,
                parent,
                quaternion: [
                    quaternion_raw.x,
                    quaternion_raw.y,
                    quaternion_raw.z,
                    quaternion_raw.w,
                ],
                transition: [translation.x, translation.y, translation.z],
            };
            out_joints_vec.push(rdjoint);
        }
    }
    out_joints_vec
}

type ReadMeshOutput = Option<(u32, VertexFormat2, Vec<Triangle>, u32, Vec<MeshInstance>)>;

fn read_mesh(
    gltf: &gltf::Document,
    buffers: &[gltf::buffer::Data],
    dst_format: TargetVertexFormat,
    read_joints: bool,
    mut negative_x_and_v0v2v1: bool,
    no_transform: bool,
    overide_mesh_idx: Option<Vec<u32>>,
) -> ReadMeshOutput {
    // only the first mesh the file gets read
    if let Some(mesh) = gltf.meshes().next() {
        info!("Mesh #{}", mesh.index());

        let mesh_instantiating_node =
            find_first_mesh_instantiating_node(gltf, mesh.index()).unwrap();
        debug!("mesh_instantiating_node: {}", mesh_instantiating_node);

        let mut base: Matrix4<f32> = if no_transform {
            Matrix4::identity()
        } else {
            build_transform2(gltf, mesh_instantiating_node)
        };

        if negative_x_and_v0v2v1 {
            let m = Matrix3::new(-1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0);
            base *= m.to_homogeneous();
            negative_x_and_v0v2v1 = false;
        }

        debug!("base: {}", &base);

        let det = base.determinant();
        if det.is_sign_negative() {
            warn!("determinant is negative: {}", det);
            warn!("negative determinant requires special code path!");
            negative_x_and_v0v2v1 = true;
        }

        let mat3 = base.resize(3, 3, 0.0);
        let inv_transform_mat3 = mat3.try_inverse().unwrap();
        let transpose_inv_transform_mat3 = inv_transform_mat3.transpose();

        let ident = match dst_format {
            TargetVertexFormat::P4h_N4b_G4b_B4b_T2h => {
                crate::vertex::p4h_n4b_g4b_b4b_t2h().to_vec()
            }
            TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_I4b => {
                crate::vertex::p4h_n4b_g4b_b4b_t2h_i4b().to_vec()
            }
            TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_I4b_W4b => {
                crate::vertex::p4h_n4b_g4b_b4b_t2h_i4b_w4b().to_vec()
            }
        };
        let vertsize = ident.iter().map(|x| x.get_size()).sum();

        let mut mesh_info: Vec<MeshInstance> = Vec::new();
        let mut merged_triangle_vec = Vec::new();
        let mut vertices_count: u32 = 0;
        let mut verts_vec = BytesMut::with_capacity(64000 * vertsize as usize);

        let mut kown_vbuffers = HashMap::new();

        if let Some(v) = overide_mesh_idx.as_ref() {
            assert_eq!(mesh.primitives().len(), v.len());
        }
        for (i, primitive) in mesh.primitives().enumerate() {
            info!("- Primitive #{}", primitive.index());
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

            let mut position_iter = reader.read_positions().unwrap();
            let mut count = position_iter.len();
            let normal_it = match reader.read_normals() {
                Some(iter) => iter.collect(),
                None => {
                    error!("Model has no normals! Enable normal export in Blender! Non existing tangents will cause garbage values!");
                    vec![[0.0f32, 0.0f32, 1.0f32]]
                }
            };
            let mut normal_iter = normal_it.into_iter().cycle();

            let tangent_it = match reader.read_tangents() {
                Some(iter) => iter.collect(),
                None => {
                    error!("Model has no tangents! Enable tangent export in Blender! Non existing tangents will cause garbage values!");
                    vec![[0.0f32, 0.0f32, 0.0f32, 1.0f32]]
                }
            };
            let mut tangent_iter = tangent_it.into_iter().cycle();

            let tex_iter1 = reader.read_tex_coords(0);
            let p: Vec<[f32; 2]> = match tex_iter1 {
                Some(tex) => {
                    let r: Vec<[f32; 2]> = tex.into_f32().collect();
                    assert_eq!(count, r.len());
                    r
                }
                None => {
                    error!("No tex_coords ! Non existing 'texcoord_0' will cause garbage values!");
                    vec![[0.0f32, 0.0f32]]
                }
            };
            let mut tex_iter = p.into_iter().cycle();

            let jvecarr: Vec<[u16; 4]> = match reader.read_joints(0) {
                Some(joints) if read_joints => {
                    let j: Vec<[u16; 4]> = joints.into_u16().collect();
                    assert_eq!(count, j.len());
                    j
                }
                _ => {
                    warn!("No joints in glTF file !");
                    if read_joints {
                        panic!("No joints in glTF file but --skeleton flag was set!")
                    }
                    vec![[0, 0, 0, 0]]
                }
            };

            let mut joints_iter = jvecarr.into_iter().cycle();

            let wvecarr: Vec<[f32; 4]> = match reader.read_weights(0) {
                Some(weight) if read_joints => {
                    let j: Vec<[f32; 4]> = weight.into_f32().collect();
                    assert_eq!(count, j.len());
                    j
                }
                _ => {
                    warn!("No weights in glTF file !");
                    if read_joints {
                        panic!("No joints/weights in glTF file but --skeleton flag was set!")
                    }
                    vec![[0.0, 0.0, 0.0, 0.0]]
                }
            };

            let mut weights_iter = wvecarr.into_iter().cycle();

            info!("dst_format: {:?}", dst_format);
            //let mut verts_vec = BytesMut::with_capacity(count * vertsize as usize);

            trace!("vertex read loop");
            let mut start_vertices_count = verts_vec.len() as u32 / vertsize;

            let pre_vertices_added = verts_vec.len();

            while count > 0 {
                trace!("count {}", count);
                let vertex_position = position_iter.next().unwrap();
                let vertex =
                    Point3::new(vertex_position[0], vertex_position[1], vertex_position[2]);
                let transformed_vertex = base.transform_point(&vertex);

                let p4h = P4h {
                    data: [
                        f16::from_f32(1.0 * transformed_vertex[0]),
                        f16::from_f32(1.0 * transformed_vertex[1]),
                        f16::from_f32(1.0 * transformed_vertex[2]),
                        f16::from_f32(0.0),
                    ],
                };

                let normals = normal_iter.next().unwrap();
                let normv = Vector3::new(normals[0], normals[1], normals[2]);
                let transformed_normals = &transpose_inv_transform_mat3 * normv;

                let mut nx = transformed_normals[0];
                let mut ny = transformed_normals[1];
                let mut nz = transformed_normals[2];
                //dbg!(((nx * nx) + (ny * ny) + (nz * nz)).sqrt());

                let len = ((nx * nx) + (ny * ny) + (nz * nz)).sqrt();

                nx /= len;
                ny /= len;
                nz /= len;

                let n4b = N4b {
                    data: [
                        (((nx + 1.0) / 2.0) * 255.0).round() as u8,
                        (((ny + 1.0) / 2.0) * 255.0).round() as u8,
                        (((nz + 1.0) / 2.0) * 255.0).round() as u8,
                        0,
                    ],
                };

                let tangents = tangent_iter.next().unwrap();
                let tangv = Vector3::new(tangents[0], tangents[1], tangents[2]);
                let transformed_tangents = &transpose_inv_transform_mat3 * tangv;

                let mut tx = transformed_tangents[0];
                let mut ty = transformed_tangents[1];
                let mut tz = transformed_tangents[2];

                //let tlen = -1.0f32*((tx * tx) + (ty * ty) + (tz * tz)).sqrt();
                //dbg!(((tx * tx) + (ty * ty) + (tz * tz)).sqrt());
                let tlen = -1.0;

                tx /= tlen;
                ty /= tlen;
                tz /= tlen;

                let tw = if negative_x_and_v0v2v1 {
                    tangents[3]
                } else {
                    -tangents[3]
                };
                //assert_relative_eq!(tw.abs(), 1.0);

                let g4b = G4b {
                    data: [
                        (((tx + 1.0) / 2.0) * 255.0).round() as u8,
                        (((ty + 1.0) / 2.0) * 255.0).round() as u8,
                        (((tz + 1.0) / 2.0) * 255.0).round() as u8,
                        0,
                    ],
                };

                let normal = Vector3::new(nx, ny, nz);
                let tangent = Vector3::new(tx, ty, tz);
                trace!("normal.dot(&tangent): {}", normal.dot(&tangent));

                let b: Matrix3x1<f32> = (normal.cross(&tangent)) * (tw);

                let b4b = B4b {
                    data: [
                        (((b.x + 1.0) / 2.0) * 255.0).round() as u8,
                        (((b.y + 1.0) / 2.0) * 255.0).round() as u8,
                        (((b.z + 1.0) / 2.0) * 255.0).round() as u8,
                        0,
                    ],
                };

                // tex

                let tex = tex_iter.next().unwrap();
                //let tex = [0.0, 0.0];
                let t2h = T2h {
                    data: [f16::from_f32(tex[0]), f16::from_f32(tex[1])],
                };

                verts_vec.put_vertex_data(&p4h);
                verts_vec.put_vertex_data(&n4b);
                verts_vec.put_vertex_data(&g4b);
                verts_vec.put_vertex_data(&b4b);
                verts_vec.put_vertex_data(&t2h);
                // TODO clean up checks
                if dst_format == TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_I4b
                    || dst_format == TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_I4b_W4b
                {
                    // joints idx
                    let joint = joints_iter.next().unwrap();

                    let i4b = I4b {
                        data: [
                            joint[0] as u8,
                            joint[1] as u8,
                            joint[2] as u8,
                            joint[3] as u8,
                        ],
                    };
                    verts_vec.put_vertex_data(&i4b);
                }

                if dst_format == TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_I4b_W4b {
                    let weight = weights_iter.next().unwrap();
                    let w4b = W4b {
                        data: [
                            (weight[0] * 255.0).round() as u8,
                            (weight[1] * 255.0).round() as u8,
                            (weight[2] * 255.0).round() as u8,
                            (weight[3] * 255.0).round() as u8,
                        ],
                    };
                    verts_vec.put_vertex_data(&w4b);
                }

                count -= 1;
            }

            let mut hasher = DefaultHasher::new();
            Hash::hash_slice(&verts_vec[pre_vertices_added..verts_vec.len()], &mut hasher);
            let hash_v = hasher.finish();
            debug!("Primivive {} hash is {:x}.", i, &hash_v);
            match kown_vbuffers.get(&hash_v) {
                Some(found_start_vertices_count) => {
                    info!("found hash {:x}! Buffer already written.", &hash_v);
                    start_vertices_count = *found_start_vertices_count;
                    verts_vec.resize(pre_vertices_added, 0)
                }
                None => {
                    kown_vbuffers.insert(hash_v, start_vertices_count);
                }
            }

            vertices_count = verts_vec.len() as u32 / vertsize;
            info!("Vertex count: {}", vertices_count);
            if vertices_count > (u16::MAX - 1).into() {
                error!("Mesh consists of too many vertices ({})!", vertices_count);
                error!("Mesh vertices count must stay below 65535!");
                error!("The vertex count that Max/Maya/Blender show may not reflect the reality of the glTF.");
                error!("E.g. during Blender's glTF export, shared vertices may need to be unshared (duplicated again) if not all vertex attributes (normals, tangents, UV) are exactly the same!")
            }

            //let verts = VertexFormat2::new(ident, vertices_count, vertsize, 0, verts_vec.freeze());

            let mut triangle_iter = reader.read_indices().unwrap().into_u32();
            let mut triangle_vec: Vec<Triangle> = Vec::with_capacity(count);

            let mut tcount = triangle_iter.len() / 3;

            while tcount > 0 {
                //let ctri = triangle_iter.next().unwrap();
                let v0 = triangle_iter.next().unwrap();
                let v1 = triangle_iter.next().unwrap();
                let v2 = triangle_iter.next().unwrap();
                let t = if negative_x_and_v0v2v1 {
                    Triangle {
                        indices: [
                            u16::try_from(start_vertices_count + v0).unwrap(),
                            u16::try_from(start_vertices_count + v2).unwrap(),
                            u16::try_from(start_vertices_count + v1).unwrap(),
                        ],
                    }
                } else {
                    Triangle {
                        indices: [
                            u16::try_from(start_vertices_count + v0).unwrap(),
                            u16::try_from(start_vertices_count + v1).unwrap(),
                            u16::try_from(start_vertices_count + v2).unwrap(),
                        ],
                    }
                };
                tcount -= 1;
                triangle_vec.push(t);
            }

            mesh_info.push(MeshInstance {
                start_index_location: merged_triangle_vec.len() as u32 * 3,
                index_count: triangle_vec.len() as u32 * 3,
                material: match overide_mesh_idx.as_ref() {
                    Some(j) => j[i],
                    None => i.try_into().unwrap(),
                },
            });

            merged_triangle_vec.append(&mut triangle_vec);

            info!("{:?}", &mesh_info);
            //return Some((vertsize, verts, merged_triangle_vec, vertices_count, mesh_info));
        }
        let verts = VertexFormat2::new(
            ident.into_boxed_slice(),
            vertices_count,
            vertsize,
            0,
            verts_vec.freeze(),
        );
        return Some((
            vertsize,
            verts,
            merged_triangle_vec,
            vertices_count,
            mesh_info,
        ));
    }
    None
}

fn find_first_mesh_instantiating_node(gltf: &gltf::Document, mesh_idx: usize) -> Option<usize> {
    for (i, node) in gltf.nodes().enumerate() {
        match node.mesh() {
            Some(mesh) => {
                debug!("mesh.index(): {}", mesh.index());
                if mesh.index() == mesh_idx {
                    return Some(i);
                }
            }
            None => continue,
        }
    }
    None
}

fn build_transform2(gltf: &gltf::Document, mesh_node: usize) -> Matrix4<f32> {
    let mut child_list: Vec<Option<Vec<usize>>> = vec![None; gltf.nodes().count()];

    let mut tmp: Vec<usize> = Vec::new();
    for (i, node) in gltf.nodes().enumerate() {
        for child in node.children() {
            tmp.push(child.index());
        }
        if !tmp.is_empty() {
            child_list[i] = Some(tmp);
            tmp = Vec::new();
        }
    }

    //let mesh_node = 1;
    let mut find_node: usize = mesh_node;
    let mut tree: Vec<usize> = Vec::new();
    assert!(find_node < child_list.len());
    loop {
        let idx = find_parent(find_node, &child_list);
        match idx {
            Some(value) => {
                tree.push(value);
                find_node = value;
            }
            None => {
                break;
            }
        }
    }

    debug!("tree: {:?}", &tree);

    calculate_global_transform(mesh_node, &tree, gltf)
}

fn calculate_global_transform(
    target_node: usize,
    tree: &[usize],
    gltf: &gltf::Document,
) -> Matrix4<f32> {
    let doc = gltf.clone();

    let nodes: Vec<gltf::scene::Node> = doc.nodes().collect();
    debug!("target_node: {} ", target_node);
    let rel_transform_data = nodes[target_node].transform().matrix();
    let mut mat4rel_transform = Matrix4::identity();
    mat4rel_transform.m11 = rel_transform_data[0][0];
    mat4rel_transform.m21 = rel_transform_data[0][1];
    mat4rel_transform.m31 = rel_transform_data[0][2];
    mat4rel_transform.m41 = rel_transform_data[0][3];

    mat4rel_transform.m12 = rel_transform_data[1][0];
    mat4rel_transform.m22 = rel_transform_data[1][1];
    mat4rel_transform.m32 = rel_transform_data[1][2];
    mat4rel_transform.m42 = rel_transform_data[1][3];

    mat4rel_transform.m13 = rel_transform_data[2][0];
    mat4rel_transform.m23 = rel_transform_data[2][1];
    mat4rel_transform.m33 = rel_transform_data[2][2];
    mat4rel_transform.m43 = rel_transform_data[2][3];

    mat4rel_transform.m14 = rel_transform_data[3][0];
    mat4rel_transform.m24 = rel_transform_data[3][1];
    mat4rel_transform.m34 = rel_transform_data[3][2];
    mat4rel_transform.m44 = rel_transform_data[3][3];

    let mut bmat4: Matrix4<f32> = Matrix4::identity();
    //dbg!(&rel_transform);
    if !tree.is_empty() {
        for p in tree.iter().rev() {
            let mat = nodes[*p].transform().matrix();
            let mut mat4: Matrix4<f32> = Matrix4::identity();
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

            bmat4 *= mat4;
        }
    }
    mat4rel_transform = bmat4 * mat4rel_transform;
    mat4rel_transform
}

fn find_parent(mesh_node: usize, child_list: &[Option<Vec<usize>>]) -> Option<usize> {
    let mut parent_idx: Option<usize> = None;
    for (i, opt_children) in child_list.iter().enumerate() {
        match opt_children {
            Some(c) => {
                let opt_parent_idx = c.iter().position(|&r| r == mesh_node);

                match opt_parent_idx {
                    Some(_) => {
                        parent_idx = Some(i);
                        break;
                    }
                    None => continue,
                }
            }
            None => continue,
        }
    }
    parent_idx
}
