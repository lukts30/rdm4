#![forbid(unsafe_code)]

use crate::{
    gltf_buffers::{AccessorBuilder, BufferContainer, GltfBuffers},
    rdm_material::RdMaterial,
    vertex::*,
    MeshInfo, RdModell,
};
use gltf::{json, json::validation::Checked::Valid, mesh::Semantic};
use std::{
    borrow::Cow,
    collections::BTreeMap,
    env,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
    str::FromStr,
};

use bytes::{BufMut, BytesMut};

mod animation;
mod scene;
mod skin;
pub use scene::build_scene;

struct RdGltfBuilder {
    binary: GltfBuffers,
    nodes: Vec<json::Node>,
    attr_map: BTreeMap<json::validation::Checked<Semantic>, json::Index<json::Accessor>>,
    idx: Option<Vec<u32>>,

    rdm: RdModell,
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
            binary: GltfBuffers::default(),
            nodes: Vec::new(),
            attr_map: BTreeMap::new(),
            rdm,
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

    fn put_rdm_anim(&mut self) {
        let joints = self.rdm.joints.as_ref().expect("animation requires joints");
        let names = joints
            .iter()
            .map(|joint| joint.name.as_str())
            .collect::<Vec<_>>();
        let clip = self
            .rdm
            .anim
            .take()
            .expect("animation was checked")
            .into_scene_animation(0, &names)
            .expect("invalid animation targets");
        self.anim_node = Some(
            animation::build(&clip, joints.len(), 0, &mut self.binary)
                .expect("cannot export animation"),
        );
    }

    fn put_joint_weight(&mut self, normalise: bool) {
        if normalise {
            self.rdm.vertex.set_weight_sum();
        }
        let n = self
            .rdm
            .vertex
            .find_component_offsets(VertexSemantic::JointIndex)
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

    fn put_joint_nodes(&mut self) {
        let joints: Vec<_> = self
            .rdm
            .joints
            .as_ref()
            .unwrap()
            .iter()
            .map(|joint| skin::GlobalJoint {
                name: &joint.name,
                parent: (joint.parent != u32::MAX).then_some(joint.parent as usize),
                translation: joint.translation,
                rotation: joint.quaternion,
            })
            .collect();
        let mut output = skin::build(&joints, &mut self.binary, 0).expect("invalid skeleton");
        self.skin = Some(output.skin);
        let mut root = mesh_node(json::Index::new(0), Some("armature".into()));
        root.children = Some(output.roots);
        root.skin = Some(json::Index::new(0));
        output.nodes.push(root);
        self.nodes = output.nodes;
    }

    fn put_vertex(&mut self) {
        let mut positions = BytesMut::with_capacity(12 * self.rdm.vertex.vertex_count as usize);
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

            for value in [x, y, z] {
                positions.put_f32_le(value);
            }
        }
        let amin = Some(json::Value::from(min));
        let amax = Some(json::Value::from(max));
        // single vertex buffer
        self.put_attr(
            BufferContainer::Bytes(positions.freeze()),
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

    fn put_material(&mut self, material_len: usize) {
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
                    e.buffer_view = Some(
                        self.binary
                            .push_buffer_view(BufferContainer::U8(buffer), None),
                    );
                }
            }
        }
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
        let accessor = self.binary.push_attribute(
            buffer,
            AccessorBuilder::new(
                count.unwrap_or(self.rdm.vertex.len()) as usize,
                acctype,
                component_type,
            )
            .bounds(amin, amax),
            target,
        );

        if let Some(semantic) = semantic {
            self.attr_map.insert(Valid(semantic), accessor);
        }
        accessor.value() as u32
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
            let mut buff = BytesMut::with_capacity(4 * submesh.index_count as usize);
            let r = (submesh.start_index_location / 3) as usize
                ..((submesh.start_index_location / 3) + submesh.index_count / 3) as usize;
            for triangle in &self.rdm.triangle_indices[r] {
                for index in triangle.indices {
                    buff.put_u32_le(index);
                }
            }
            bytes.push((BufferContainer::Bytes(buff.freeze()), submesh.index_count));
        }
        for b in bytes.into_iter() {
            let acc = self.put_attr(
                b.0,
                json::accessor::Type::Scalar,
                json::accessor::ComponentType::U32,
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

    fn mesh(&self, name: Option<String>) -> json::Mesh {
        // put_material must already have been run otherwise this panics!
        let mats = self.material_idx.as_ref().unwrap();
        let indices_vec = self.idx.as_ref().unwrap();
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

        json::Mesh {
            extensions: Default::default(),
            extras: Default::default(),
            name,
            primitives: triangle_vec,
            weights: None,
        }
    }

    fn build(mut self, single_buffer: bool) -> RdGltf {
        let mesh = self.mesh(None);

        // if nodes vec is non empty than put_joint_nodes already added a scene root node to the end
        if self.nodes.is_empty() {
            let scene_root_node = mesh_node(json::Index::new(0), None);
            self.nodes.push(scene_root_node);
        }

        // get index of last node (scene root node)
        let root_node_idx = json::Index::new((self.nodes.len() - 1) as u32);

        self.finish(vec![mesh], vec![root_node_idx], single_buffer)
    }

    fn finish(
        self,
        meshes: Vec<json::Mesh>,
        scene_nodes: Vec<json::Index<json::Node>>,
        single_buffer: bool,
    ) -> RdGltf {
        let animation = self.anim_node.map_or(Default::default(), |n| vec![n]);
        let mut root = json::Root {
            meshes,
            nodes: self.nodes,
            scene: Some(json::Index::new(0)),
            scenes: vec![json::Scene {
                extensions: Default::default(),
                extras: Default::default(),
                name: None,
                nodes: scene_nodes,
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

        let buffers = self.binary.finish(&mut root, single_buffer);
        RdGltf { buffers, root }
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
        b.put_material(MeshInfo::get_max_material(&b.rdm.mesh_info) as usize + 1);

        b.put_normal();
        b.put_tangent();

        if has_skin {
            b.put_joint_nodes();
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
    let p = b.build(config != GltfExportFormat::GltfSeparate);
    info!("Built glTF");
    p.write_gltf(dir, mat_opt, create_new, config);
}

fn mesh_node(mesh: json::Index<json::Mesh>, name: Option<String>) -> json::Node {
    json::Node {
        camera: None,
        children: None,
        extensions: Default::default(),
        extras: Default::default(),
        matrix: None,
        mesh: Some(mesh),
        name,
        rotation: None,
        scale: None,
        translation: None,
        skin: None,
        weights: None,
    }
}

struct RdGltf {
    buffers: Vec<BufferContainer>,
    root: json::Root,
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
    fn write_gltf(
        mut self,
        dir: Option<PathBuf>,
        optmat: Option<RdMaterial>,
        create_new: bool,
        config: GltfExportFormat,
    ) {
        if config == GltfExportFormat::Glb {
            self.root.buffers[0].uri = None;
        }
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
        info!("Writing glTF {:?}", file);
        let output_path = file.clone();

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
                let j = json::serialize::to_vec(&self.root).expect("Serialization error");
                let glb = gltf::Glb {
                    header,
                    json: Cow::from(&j),
                    bin: Some(Cow::from(self.buffers[0].get_bytes())),
                };
                glb.to_writer(writer).expect("I/O error");
                debug!("json: {}", glb.json.len());
                debug!("bin: {}", self.buffers[0].get_bytes_len_padded());
            }
            _ => {
                let vjson =
                    json::serialize::to_vec_pretty(&self.root).expect("Serialization error");
                writer.write_all(&vjson).expect("I/O error");

                debug!("wrote json to disk!");

                file.pop();
                let udir = file;

                for (i, bin) in self.buffers.into_iter().enumerate() {
                    let mut file_path = udir.clone();
                    file_path.push(format!("buffer{}.bin", i));
                    debug!("write_all {:?}", file_path);
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
                        debug!("copy: {:?} to {:?}", src, dst_file);
                        fs::copy(src, &dst_file).unwrap();
                    }
                }
            }
        }
        info!("Wrote glTF {:?}", output_path);
    }
}
