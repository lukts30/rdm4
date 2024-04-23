use half::f16;
use nalgebra::Vector3;
use crate::vertex::*;
use crate::vertex::W4b;
use nalgebra::*;

pub struct TransformContext {
    pub(crate) base: Matrix<f32, Const<4>, Const<4>, ArrayStorage<f32, 4, 4>>,
    pub(crate) transpose_inv_transform_mat3 : Matrix<f32, Const<3>, Const<3>, ArrayStorage<f32, 3, 3>>,
}

impl TransformContext {    
    pub fn transform_position(&mut self, position : [f32; 3]) -> Point<f32, 3> {
        let vertex = Point3::new(position[0], position[1], position[2]);
        return self.base.transform_point(&vertex)
    }

    pub fn transform_tangent(&mut self, tangents: [f32; 4]) -> Vector3<f32> {
        let tangv = Vector3::new(tangents[0], tangents[1], tangents[2]);
        let transformed_tangents = self.transpose_inv_transform_mat3 * tangv;

        let mut tx = transformed_tangents[0];
        let mut ty = transformed_tangents[1];
        let mut tz = transformed_tangents[2];
        let tlen = -1.0;
        tx /= tlen;
        ty /= tlen;
        tz /= tlen;

        return Vector3::new(tx, ty, tz); 
    }

    pub fn transform_normal(&mut self, normal : [f32; 3]) -> Vector3<f32>{
        let normv: Vector3<f32> = Vector3::new(normal[0], normal[1], normal[2]);
        let transformed_normal: Vector3<f32> = self.transpose_inv_transform_mat3 * normv;

        let mut nx = transformed_normal[0];
        let mut ny = transformed_normal[1];
        let mut nz = transformed_normal[2];

        let len = ((nx * nx) + (ny * ny) + (nz * nz)).sqrt();

        nx /= len;
        ny /= len;
        nz /= len;

        return Vector3::new(nx, ny, nz)
    }
}



// # Position #
pub fn p4h(position: Point<f32, 3>) -> P4h {
    P4h {
        data: [
            f16::from_f32(1.0 * position[0]),
            f16::from_f32(1.0 * position[1]),
            f16::from_f32(1.0 * position[2]),
            f16::from_f32(0.0),
        ],  
    }
}

pub fn p3f(position: Point<f32, 3>) -> P3f {
    P3f {
        data: [
            1.0 * position[0],
            1.0 * position[1],
            1.0 * position[2]
        ]
    }
}


// # Normals #

pub fn n4b(vec_normal: Vector3<f32>) -> N4b {
    N4b {
        data: [
            (((vec_normal.x + 1.0) / 2.0) * 255.0).round() as u8,
            (((vec_normal.y + 1.0) / 2.0) * 255.0).round() as u8,
            (((vec_normal.z + 1.0) / 2.0) * 255.0).round() as u8,
            0,
        ],
    }
}

pub fn n3f(vec_normal: Vector3<f32>)  -> N3f {
    N3f {
        data: [
            vec_normal.x,
            vec_normal.y,
            vec_normal.z
        ]
    }
}

// # Tangents #
pub fn g4b(vec_tangent: Vector3<f32>) -> G4b {
    G4b {
        data: [
            (((vec_tangent.x + 1.0) / 2.0) * 255.0).round() as u8,
            (((vec_tangent.y + 1.0) / 2.0) * 255.0).round() as u8,
            (((vec_tangent.z + 1.0) / 2.0) * 255.0).round() as u8,
            0,
        ]
    }
}

pub fn g3f(vec_tangent: Vector3<f32>) -> G3f {
    G3f {
        data: [
            vec_tangent.x,
            vec_tangent.y,
            vec_tangent.z
        ]
    }
}

// # Bitangents #
pub fn b4b(vec_tangent: Vector3<f32>, vec_normal: Vector3<f32>, tangent_w: f32) -> B4b {
    debug!("normal.dot(&tangent): {}", vec_normal.dot(&vec_tangent));
    let b: Matrix3x1<f32> = (vec_normal.cross(&vec_tangent)) * (tangent_w);

    B4b {
        data: [
            (((b.x + 1.0) / 2.0) * 255.0).round() as u8,
            (((b.y + 1.0) / 2.0) * 255.0).round() as u8,
            (((b.z + 1.0) / 2.0) * 255.0).round() as u8,
            0,
        ],
    }
}

pub fn b3f(vec_tangent: Vector3<f32>, vec_normal: Vector3<f32>, tangent_w: f32) -> B3f {
    debug!("normal.dot(&tangent): {}", vec_normal.dot(&vec_tangent));
    let b: Matrix3x1<f32> = (vec_normal.cross(&vec_tangent)) * (tangent_w);

    B3f {
        data: [
            b.x,
            b.y,
            b.z
        ]
    }
}

// # UV # 
pub fn t2h(uv : [f32; 2]) -> T2h {
    T2h {
        data: [
            f16::from_f32(uv[0]), f16::from_f32(uv[1])
        ]
    }
}

pub fn t2f(uv: [f32; 2]) -> T2f {
    T2f {
        data: [
            uv[0],
            uv[1]
        ]
    }
}

// # Joints # 
pub fn i4b(joint: [u16; 4]) -> I4b {
    I4b {
        data: [
            joint[0] as u8,
            joint[1] as u8,
            joint[2] as u8,
            joint[3] as u8,
        ],
    }
}

// # Weights # 
pub fn w4b(weight: [f32; 4]) -> W4b {
    W4b {
        data: [
            (weight[0] * 255.0).round() as u8,
            (weight[1] * 255.0).round() as u8,
            (weight[2] * 255.0).round() as u8,
            (weight[3] * 255.0).round() as u8,
        ],
    }
}

pub fn c4b(color: [u8; 4]) -> C4b {
    C4b {
        data: color,
    }
}

pub fn c4c(color: [u8; 4]) -> C4c {
    C4c {
        data: [
            (color[0] as i16 - 128) as i8,
            (color[1] as i16 - 128) as i8,
            (color[2] as i16 - 128) as i8,
            (color[3] as i16 - 128) as i8,
        ],
    }
}
