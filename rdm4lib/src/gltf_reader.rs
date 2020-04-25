use gltf;

use crate::RDJoint;
use crate::RDModell;
use crate::Triangle;
use crate::VertexFormat;

use crate::N4b;
use crate::P4h;
use crate::G4b;

use half::f16;

#[test]
pub fn start() {
    let (gltf, buffers, _) = gltf::import("triangle/triangle.gltf").unwrap();
    for mesh in gltf.meshes() {
        println!("Mesh #{}", mesh.index());
        for primitive in mesh.primitives() {
            println!("- Primitive #{}", primitive.index());
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

            let mut position_iter = reader.read_positions().unwrap();
            let mut normal_iter = reader.read_normals().unwrap();
            let mut tangent_iter = reader.read_tangents().unwrap();

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

                let normals = normal_iter.next().unwrap();
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

                let tangents = tangent_iter.next().unwrap();
                let tx = tangents[0];
                let ty = tangents[1];
                let tz = tangents[2];
                let tw = tangents[3];

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

                verts_vec.push(VertexFormat::P4h(p4h));

                count = count - 1;
            }

            println!("verts_vec {}", verts_vec.len());
        }
    }
}
