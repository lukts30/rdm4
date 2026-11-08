//! Command execution, independent of CLI argument parsing.

use std::{
    error::Error,
    fs::{self, File},
    io::{self, BufReader, Write},
    path::{Path, PathBuf},
};

use log::{error, info};

use crate::reporting;
use rdm4lib::{
    gltf_export::{self, GltfExportFormat},
    gltf_reader::{AnimationImportOptions, ImportedGltf, ResolveNodeName},
    rdm::{
        write_scene, RdmAnimation, RdmDocument, RdmEndian, RdmExportOptions, RdmReader,
        RdmScanReport, RdmScanner, RdmSplitter,
    },
    rdm_material::RdMaterial,
    vertex::TargetVertexFormat,
};

/// Split model records into indexed RDM files and print their paths.
pub fn split(
    input: &Path,
    directory: &Path,
    force: bool,
    mut output: impl Write,
) -> Result<(), Box<dyn Error>> {
    let file = File::open(input)?;
    reporting::input(input, file.metadata()?.len());
    let mut splitter = RdmSplitter::new(BufReader::new(file))?;
    info!(
        "Splitting {} models into {}; shared records and source bytes are preserved",
        splitter.model_count(),
        directory.display()
    );
    let paths: Vec<_> = (0..splitter.model_count())
        .map(|index| {
            let mut name = input
                .file_stem()
                .expect("input has a filename")
                .to_os_string();
            name.push(format!(".model-{index:03}.rdm"));
            directory.join(name)
        })
        .collect();
    for path in &paths {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.is_dir() => {
                return Err(format!("output is a directory: {}", path.display()).into())
            }
            Ok(_) if !force => {
                return Err(format!(
                    "output exists: {} (use --force to replace files)",
                    path.display()
                )
                .into())
            }
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error.into()),
            _ => {}
        }
    }
    fs::create_dir_all(directory)?;
    for (index, path) in paths.iter().enumerate() {
        let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
        info!(
            "Writing model #{index} ({}/{}) to {}",
            index + 1,
            paths.len(),
            path.display()
        );
        splitter.write_model(index, &mut temporary)?;
        let bytes = temporary.as_file().metadata()?.len();
        if force {
            temporary.persist(path)?;
        } else {
            temporary.persist_noclobber(path)?;
        }
        info!("Wrote model #{index}: {} ({} bytes)", path.display(), bytes);
        writeln!(output, "{}", path.display())?;
    }
    info!(
        "Split complete: {} files written to {}",
        paths.len(),
        directory.display()
    );
    Ok(())
}

/// Output representation for inspection reports.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum InspectFormat {
    /// Human-readable summary and block table.
    Text,
    /// Structured report with numeric byte offsets.
    Json,
}

impl InspectFormat {
    /// Render an existing scan report without reading the source again.
    pub fn write(self, input: &Path, report: &RdmScanReport, output: impl Write) -> io::Result<()> {
        self.write_with_records(input, report, None, output)
    }

    fn write_with_records(
        self,
        input: &Path,
        report: &RdmScanReport,
        document: Option<&RdmDocument>,
        mut output: impl Write,
    ) -> io::Result<()> {
        match self {
            Self::Text => {
                inspect_text(input, report, &mut output)?;
                if let Some(document) = document {
                    let models = document.models.as_deref().unwrap_or_default();
                    writeln!(output, "Models: {}", models.len())?;
                    for (i, model) in models.iter().enumerate() {
                        writeln!(
                            output,
                            "  [{i}] {}: {} vertices, {} indices, {} groups; {}",
                            model.name.name.as_deref().unwrap_or("<unnamed>"),
                            model.vertices.count,
                            model.indices.values.len(),
                            model.groups.len(),
                            model.vertex_format
                        )?;
                    }
                    writeln!(
                        output,
                        "Materials: {}; joints: {}; animations: {}",
                        document.materials.as_ref().map_or(0, Vec::len),
                        document.skin.as_ref().map_or(0, |skin| skin.joints.len()),
                        document.animations.as_ref().map_or(0, Vec::len)
                    )?;
                }
                Ok(())
            }
            Self::Json => {
                use serde_json::json;

                let range = |r: &std::ops::Range<usize>| json!({ "start": r.start, "end": r.end });
                let value = json!({
                    "schema_version": 1,
                    "file": input.to_string_lossy(),
                    "source_len": report.source_len,
                    "endian": match report.endian {
                        Some(RdmEndian::Little) => Some("little"),
                        Some(RdmEndian::Big) => Some("big"),
                        None => None,
                    },
                    "preamble": report.preamble,
                    "framing_complete": report.unscanned.is_none(),
                    "blocks": report.blocks.iter().map(|block| json!({
                        "offset": block.offset, "count": block.count, "stride": block.stride,
                        "payload": range(&block.payload),
                    })).collect::<Vec<_>>(),
                    "unscanned": report.unscanned.as_ref().map(range),
                    "issues": report.issues.iter().map(|issue| json!({
                        "offset": issue.offset(), "context": issue.context(),
                        "expected": issue.expected(), "actual": issue.actual(),
                    })).collect::<Vec<_>>(),
                    "records": document.map(record_summary),
                });
                serde_json::to_writer_pretty(&mut output, &value)?;
                writeln!(output)
            }
        }
    }
}

/// Inspect framing and optionally decode all known records and buffers.
/// Format errors retain the framing report and return false.
pub fn inspect(
    input: &Path,
    format: InspectFormat,
    records: bool,
    output: impl Write,
) -> io::Result<bool> {
    let mut source = BufReader::new(File::open(input)?);
    let mut report = RdmScanner::new(&mut source).scan()?;
    let mut document = None;
    if records && report.issues.is_empty() && report.unscanned.is_none() {
        match RdmReader::new(source).and_then(|reader| reader.read_root::<RdmDocument>()) {
            Ok(decoded) => document = Some(decoded.value),
            Err(error) => report.issues.push(error),
        }
    }
    format.write_with_records(input, &report, document.as_ref(), output)?;
    Ok(report.issues.is_empty() && report.unscanned.is_none())
}

fn record_summary(document: &RdmDocument) -> serde_json::Value {
    use serde_json::json;
    json!({
        "export_info": document.export_info.as_ref().map(|info| json!({
            "source_path": info.source_path, "rmp_name": info.rmp_name,
        })),
        "models": document.models.iter().flatten().map(|model| json!({
            "name": model.name.name, "vertex_format": model.vertex_format.to_string(),
            "vertex_count": model.vertices.count, "index_count": model.indices.values.len(),
            "index_storage": format!("{:?}", model.index_format.storage_type),
            "skeleton_index": model.skeleton_index,
            "bounds_min": model.bounds_min, "bounds_max": model.bounds_max,
            "groups": model.groups.iter().map(|group| json!({
                "first_index": group.first_index, "index_count": group.index_count,
                "material": group.material,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "materials": document.materials.iter().flatten().map(|entry| json!({
            "name": entry.material.name, "texture": entry.material.texture,
        })).collect::<Vec<_>>(),
        "joint_count": document.skin.as_ref().map_or(0, |skin| skin.joints.len()),
        "animations": document.animations.iter().flatten().map(|animation| json!({
            "name": animation.name, "track_count": animation.tracks.len(),
            "time_max": animation.time_max, "sampling_rate": animation.sampling_rate,
        })).collect::<Vec<_>>(),
    })
}

fn inspect_text(input: &Path, report: &RdmScanReport, mut output: impl Write) -> io::Result<()> {
    let order = match report.endian {
        Some(RdmEndian::Little) => "little",
        Some(RdmEndian::Big) => "big",
        None => "unknown",
    };
    writeln!(output, "File: {}", input.display())?;
    writeln!(output, "Size: {} bytes", report.source_len)?;
    writeln!(output, "Byte order: {order}")?;
    if let Some(words) = report.preamble {
        writeln!(output, "Preamble (0x04..0x14): {words:#010x?}")?;
    }
    writeln!(
        output,
        "Blocks: {} (physical framing only)",
        report.blocks.len()
    )?;
    writeln!(
        output,
        "  Header      Payload start..end        Count    Stride"
    )?;
    for block in &report.blocks {
        writeln!(
            output,
            "  {:#010x}  {:#010x}..{:#010x}  {:>8}  {:>8}",
            block.offset, block.payload.start, block.payload.end, block.count, block.stride,
        )?;
    }
    if let Some(range) = &report.unscanned {
        writeln!(output, "Unscanned: {:#x}..{:#x}", range.start, range.end)?;
    }
    writeln!(output, "Issues: {}", report.issues.len())?;
    for issue in &report.issues {
        writeln!(output, "  {issue}")?;
    }
    Ok(())
}

/// Input and output settings common to both conversion directions.
pub struct Files {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub input_name: bool,
    pub force: bool,
}

impl Files {
    fn conversion_destination(
        &self,
        extension: &str,
        legacy: bool,
    ) -> Result<Option<PathBuf>, Box<dyn Error>> {
        if legacy {
            return Ok(self.destination(extension));
        }
        let mut output = self
            .output
            .clone()
            .unwrap_or_else(|| self.input.with_extension(extension));
        if self.input_name && !output.is_dir() {
            return Err("--input-name requires an existing output directory".into());
        }
        if output.is_dir() {
            output.push(self.input.file_name().ok_or("input has no filename")?);
        } else if let Some(ext) = output.extension().and_then(|ext| ext.to_str()) {
            if !ext.eq_ignore_ascii_case(extension) {
                return Err(format!("output extension .{ext} does not match .{extension}; change --format or --output").into());
            }
        }
        output.set_extension(extension);
        Ok(Some(output))
    }

    fn destination(&self, extension: &str) -> Option<PathBuf> {
        let mut output = self.output.clone();
        if let Some(ref mut path) = output {
            if self.input_name {
                assert!(
                    path.is_dir(),
                    "in_is_out_filename: output must not be a file!"
                );
                path.push(self.input.file_stem().unwrap());
                path.set_extension(extension);
            }
        }
        output
    }
}

/// Settings for RDM to glTF/GLB conversion.
pub struct GltfOptions {
    pub files: Files,
    /// Preserve the original CLI's attribute and skeleton export behavior.
    pub legacy: bool,
    pub skeleton: bool,
    pub animation: Option<PathBuf>,
    pub format: GltfExportFormat,
    pub textures: Option<Vec<PathBuf>>,
}

/// Settings for glTF to RDM conversion.
pub struct RdmOptions {
    pub files: Files,
    pub format: TargetVertexFormat,
    pub mesh_index: Option<u32>,
    /// Preserve the old CLI's metadata and per-primitive material defaults.
    pub legacy: bool,
    pub skeleton: bool,
    pub animation: bool,
    pub sampling_rate: Option<u32>,
    pub interpolate_animations: bool,
    pub no_transform: bool,
    pub mirror_x: bool,
    pub material_indices: Option<Vec<u32>>,
    pub joint_names: ResolveNodeName,
}

/// Convert all RDM models or one explicitly selected model through the shared scene.
pub fn gltf(opts: GltfOptions, mesh_index: Option<usize>) -> Result<(), Box<dyn Error>> {
    let extension = if opts.format == GltfExportFormat::Glb {
        "glb"
    } else {
        "gltf"
    };
    let output = opts.files.conversion_destination(extension, opts.legacy)?;
    let mut document = read_document(&opts.files.input)?;
    reporting::available(&document);
    if let Some(index) = mesh_index {
        let models = document.models.take().unwrap_or_default();
        let count = models.len();
        let model = models
            .into_iter()
            .nth(index)
            .ok_or_else(|| format!("model index {index} out of range for {count} models"))?;
        info!("Selected model #{index}");
        document.models = Some(vec![model]);
    }
    reporting::models(&document, mesh_index);
    if !opts.skeleton && document.skin.is_some() {
        info!("Skeleton omitted; use --skeleton to include it");
    }
    let mut scene = document.into_scene(opts.skeleton)?;
    // The old CLI activates the selected model's skeleton explicitly.
    if opts.legacy && opts.skeleton {
        scene.meshes[0].skeleton = Some(0);
    }
    if let Some(path) = &opts.animation {
        if scene.skeletons.is_empty() {
            return Err("animation requires a skeleton".into());
        }
        scene.attach_rdm_animation(read_animation(path)?, 0)?;
    }
    reporting::scene(&scene);
    let mut writer = gltf_export::GltfWriter::new(scene);
    if opts.legacy {
        writer = writer.legacy();
    }
    if let Some(textures) = opts.textures {
        info!("Converting {} DDS textures with texconv", textures.len());
        writer = writer.textures(RdMaterial::new(textures));
    }
    let mut path = output.unwrap_or_else(|| PathBuf::from("gltf_out"));
    if opts.files.output.is_none() && opts.legacy {
        fs::create_dir_all(&path)?;
    }
    if path.is_dir() {
        path.push("out");
    }
    path.set_extension(extension);
    info!("Writing glTF {}", path.display());
    writer.write(Some(path.clone()), !opts.files.force, opts.format)?;
    reporting::written(&path);
    Ok(())
}

fn read_animation(path: &Path) -> Result<RdmAnimation, Box<dyn Error>> {
    let mut animations = read_document(path)?.animations.unwrap_or_default();
    if animations.len() != 1 {
        return Err(format!(
            "animation export requires one animation, found {}",
            animations.len()
        )
        .into());
    }
    let mut animation = animations.pop().expect("one animation was checked");
    // Both CLIs retain the external clip's filename as its exported name.
    animation.name = path
        .file_stem()
        .expect("animation path has a filename")
        .to_string_lossy()
        .into_owned();
    Ok(animation)
}

fn read_document(path: &Path) -> Result<RdmDocument, Box<dyn Error>> {
    let file = File::open(path)?;
    reporting::input(path, file.metadata()?.len());
    let reader = RdmReader::new(BufReader::new(file))?;
    info!("Byte order: {:?}", reader.endian());
    Ok(reader.read_root::<RdmDocument>()?.value)
}

/// Convert selected geometry or all active-scene mesh nodes, optionally with skin and animation.
pub fn rdm(opts: RdmOptions) -> Result<(), Box<dyn Error>> {
    let output = opts.files.conversion_destination("rdm", opts.legacy)?;
    reporting::input(&opts.files.input, fs::metadata(&opts.files.input)?.len());
    let mut gltf = ImportedGltf::load(opts.files.input.as_path(), opts.joint_names)?;
    let animations = (opts.skeleton && opts.animation).then_some(AnimationImportOptions {
        interpolate: opts.interpolate_animations,
        sampling_rate: opts.sampling_rate,
        ..Default::default()
    });
    let scene = if let Some(index) = opts.mesh_index {
        gltf.select_mesh(index)?;
        gltf.read_scene(opts.skeleton, animations)?
    } else {
        gltf.read_active_scene(opts.skeleton, animations)?
    };
    info!("Destination vertex format: {:?}", opts.format);
    reporting::meshes(&scene);
    reporting::scene(&scene);
    let material_indices = if opts.legacy {
        Some(
            opts.material_indices
                .unwrap_or_else(|| (0..scene.meshes[0].primitives.len() as u32).collect()),
        )
    } else {
        opts.material_indices
    };
    let animation_count = scene.animations.len();
    let directory_output = output.as_ref().is_none_or(|path| path.is_dir());
    info!("Writing RDM and {animation_count} animation files");
    let path = write_scene(
        scene,
        RdmExportOptions {
            format: opts.format,
            mirror_x: opts.mirror_x,
            no_transform: opts.no_transform,
            material_indices,
            legacy: opts.legacy,
        },
        output,
        !opts.files.force,
    )?;
    for index in 0..animation_count {
        let name = if directory_output {
            format!("anim_{index}.rdm")
        } else {
            format!(
                "{}_anim_{index}.rdm",
                path.file_stem()
                    .expect("output has a filename")
                    .to_string_lossy()
            )
        };
        reporting::written(&path.with_file_name(name));
    }
    reporting::written(&path);
    if opts.skeleton && !opts.no_transform {
        error!("glTF skeleton is set, but no_transform is not! Animation & Mesh might be severely deformed! Use --no_transform and apply rotation & translation in the cfg file.");
    }
    Ok(())
}
