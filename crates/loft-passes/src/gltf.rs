//! Stage-0 glTF handle (06 §18). Asset indices address the whole bundle.
use loft_abi::MeshAsset;
use loft_ir::{Code, Diag, Res, Span};
use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub struct GltfScene {
    pub meshes: BTreeMap<String, MeshAsset>,
}

impl GltfScene {
    pub fn mesh(&self, name: &str, site: Span) -> Res<MeshAsset> {
        self.meshes.get(name).copied().ok_or_else(|| {
            Diag::new(
                Code::E602,
                site,
                format!("no imported mesh named {name:?}"),
                "use a mesh name from this glTF scene",
            )
        })
    }
}

use crate::{bvh::{Bvh2, Ray}, program::Instance};
use loft_parse::ast::*;
use serde::Deserialize;
use std::{fs, path::{Path, PathBuf}};

type Point = [f32; 3];
type Triangle = [Point; 3];

pub(crate) struct Mesh {
    pub triangles: Vec<Triangle>,
    tree: Bvh2,
    pub lo: Point,
    pub hi: Point,
    pub color: Point,
    pub roughness: f32,
    pub metal: f32,
    // The end triangle index is exclusive. Each primitive shares one material.
    materials: Vec<(usize, Factors)>,
}

impl Mesh {
    fn new(triangles: Vec<Triangle>, materials: Vec<(usize, Factors)>) -> Result<Self, String> {
        if triangles.is_empty() { return Err("mesh contains no triangles".into()); }
        let mut bounds = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
        let instances = triangles.iter().enumerate().map(|(row, triangle)| {
            let lo = std::array::from_fn(|axis| triangle.iter().map(|p| p[axis]).fold(f32::INFINITY, f32::min));
            let hi = std::array::from_fn(|axis| triangle.iter().map(|p| p[axis]).fold(f32::NEG_INFINITY, f32::max));
            for axis in 0..3 {
                bounds.0[axis] = bounds.0[axis].min(lo[axis]);
                bounds.1[axis] = bounds.1[axis].max(hi[axis]);
            }
            Instance { kind: 0, row: row as u32, lo, hi }
        }).collect::<Vec<_>>();
        let tree = Bvh2::build(&instances).map_err(|e| format!("triangle bounds: {e:?}"))?;
        let Factors { color, roughness, metal } = materials[0].1;
        Ok(Self { triangles, tree, lo: bounds.0, hi: bounds.1, color, roughness, metal, materials })
    }

    pub fn hit(&self, origin: Point, dir: Point) -> f32 {
        let Ok(ray) = Ray::new(origin, dir, 0.0, f32::INFINITY) else { return -1.; };
        self.tree.trace(ray, |leaf, _| {
            let [a, b, c] = self.triangles[leaf.row_id as usize];
            let edge_a = sub(b, a);
            let edge_b = sub(c, a);
            let p = cross(dir, edge_b);
            let det = dot(edge_a, p);
            if det == 0.0 { return Ok(None); }
            let s = sub(origin, a);
            let u = dot(s, p) / det;
            let q = cross(s, edge_a);
            let v = dot(dir, q) / det;
            Ok::<_, std::convert::Infallible>((u >= 0. && v >= 0. && u + v <= 1.).then(|| dot(edge_b, q) / det))
        }).unwrap().map_or(-1., |(_, t)| t)
    }
}

fn sub(a: Point, b: Point) -> Point { std::array::from_fn(|i| a[i] - b[i]) }
fn dot(a: Point, b: Point) -> f32 { a.into_iter().zip(b).map(|(x, y)| x * y).sum() }
fn cross([x, y, z]: Point, [a, b, c]: Point) -> Point { [y * c - z * b, z * a - x * c, x * b - y * a] }

pub(crate) fn words(meshes: &[Mesh]) -> Vec<f32> {
    let mut data = vec![0.; 8 * meshes.len()];
    for (i, mesh) in meshes.iter().enumerate() {
        let off = data.len() as u32;
        data[i * 8..i * 8 + 8].copy_from_slice(&[
            f32::from_bits(off), f32::from_bits(mesh.triangles.len() as u32),
            mesh.color[0], mesh.color[1], mesh.color[2], mesh.roughness, mesh.metal, 0.,
        ]);
        data.extend(mesh.triangles.iter().flatten().flatten().copied());
        // These offsets are private to the emitted mesh module and its LUT.
        // The header's inline material keeps the single-primitive path direct.
        let header = data.len();
        data[i * 8 + 7] = f32::from_bits(header as u32);
        let nodes = header + 3;
        let leaves = nodes + mesh.tree.nodes().len() * 8;
        let materials = leaves + mesh.tree.leaves().len();
        data.extend([
            f32::from_bits(nodes as u32),
            f32::from_bits(leaves as u32),
            f32::from_bits(if mesh.materials.len() > 1 { materials as u32 } else { 0 }),
        ]);
        for node in mesh.tree.nodes() {
            data.extend(node.lo);
            data.extend(node.hi);
            data.extend([f32::from_bits(node.left_or_first), f32::from_bits(node.count)]);
        }
        data.extend(mesh.tree.leaves().iter().map(|leaf| f32::from_bits(leaf.row_id)));
        if mesh.materials.len() > 1 {
            data.push(f32::from_bits(mesh.materials.len() as u32));
            for &(end, f) in &mesh.materials {
                data.push(f32::from_bits(end as u32));
                data.extend(f.color);
                data.extend([f.roughness, f.metal]);
            }
        }
    }
    data
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Document {
    asset: Asset,
    buffers: Vec<Buffer>,
    buffer_views: Vec<View>,
    accessors: Vec<Accessor>,
    meshes: Vec<GltfMesh>,
    #[serde(default)] materials: Vec<Material>,
    #[serde(default)] extensions_used: Vec<String>,
    #[serde(default)] skins: Vec<serde_json::Value>,
    #[serde(default)] animations: Vec<serde_json::Value>,
}
#[derive(Deserialize)]
struct Asset { version: String }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Buffer { byte_length: usize, uri: Option<String> }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct View {
    buffer: usize,
    #[serde(default)] byte_offset: usize,
    byte_length: usize,
    byte_stride: Option<usize>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Accessor {
    buffer_view: usize,
    #[serde(default)] byte_offset: usize,
    component_type: u32,
    count: usize,
    #[serde(rename="type")] shape: String,
    sparse: Option<serde_json::Value>,
}
#[derive(Deserialize)]
struct GltfMesh { name: Option<String>, primitives: Vec<Primitive> }
#[derive(Deserialize)]
struct Primitive {
    attributes: BTreeMap<String, usize>,
    indices: Option<usize>,
    material: Option<usize>,
    mode: Option<u32>,
    targets: Option<serde_json::Value>,
}
#[derive(Clone, Copy)]
struct Factors {
    color: Point,
    roughness: f32,
    metal: f32,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Material {
    pbr_metallic_roughness: Option<Pbr>,
    normal_texture: Option<serde_json::Value>,
    emissive_texture: Option<serde_json::Value>,
    emissive_factor: Option<Point>,
    alpha_mode: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pbr {
    base_color_factor: Option<[f32; 4]>,
    roughness_factor: Option<f32>,
    metallic_factor: Option<f32>,
    base_color_texture: Option<serde_json::Value>,
    metallic_roughness_texture: Option<serde_json::Value>,
}

fn chunks(bytes: &[u8]) -> Result<(&[u8], &[u8]), String> {
    let word = |i| bytes.get(i..i + 4).map(|x| u32::from_le_bytes(x.try_into().unwrap()));
    if word(4) != Some(2) || word(8) != u32::try_from(bytes.len()).ok() {
        return Err("invalid glTF binary header".into());
    }
    let mut json = None;
    let mut binary = None;
    let mut start = 12;
    while start < bytes.len() {
        let len = word(start).ok_or("truncated glTF chunk")? as usize;
        let tag = word(start + 4).ok_or("truncated glTF chunk")?;
        let data = bytes.get(start + 8..).and_then(|x| x.get(..len)).ok_or("truncated glTF chunk data")?;
        match tag {
            0x4e4f534a if json.is_none() => json = Some(data),
            0x004e4942 if binary.is_none() => binary = Some(data),
            _ => return Err("unsupported or duplicate glTF chunk".into()),
        }
        start += 8 + len;
    }
    Ok((json.ok_or("no glTF JSON chunk")?, binary.unwrap_or(&[])))
}

fn relative(path: &Path, value: &str) -> Result<PathBuf, String> {
    let child = Path::new(value);
    if child.is_absolute() || value.contains(':') {
        return Err("glTF paths must be local and relative to the containing file".into());
    }
    Ok(path.parent().unwrap_or(Path::new("")).join(child))
}

fn load(path: &Path, out: &mut Vec<Mesh>) -> Result<GltfScene, String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let (json, binary) = if bytes.starts_with(b"glTF") { chunks(&bytes)? } else { (&bytes[..], &[][..]) };
    let doc: Document = serde_json::from_slice(json).map_err(|e| e.to_string())?;
    if doc.asset.version != "2.0" || !doc.extensions_used.is_empty() || !doc.skins.is_empty() || !doc.animations.is_empty() {
        return Err("import supports static glTF 2.0 meshes without extensions, skins or animation".into());
    }
    let buffers = doc.buffers.iter().enumerate().map(|(i, b)| {
        let data = match &b.uri {
            Some(uri) => fs::read(relative(path, uri)?).map_err(|e| e.to_string())?,
            None if i == 0 => binary.to_vec(),
            None => return Err("glTF buffer has no source".into()),
        };
        if data.len() < b.byte_length { return Err("truncated glTF buffer".into()); }
        Ok(data)
    }).collect::<Result<Vec<_>, String>>()?;
    let data = |index: usize, len: usize, shape: &str| {
        let a = doc.accessors.get(index).ok_or("missing accessor")?;
        let v = doc.buffer_views.get(a.buffer_view).ok_or("missing buffer view")?;
        if a.sparse.is_some() || a.shape != shape { return Err("unsupported accessor layout"); }
        let buf = buffers.get(v.buffer).ok_or("missing buffer")?;
        let end = v.byte_offset.checked_add(v.byte_length).ok_or("buffer view overflows")?;
        let view = buf.get(v.byte_offset..end).ok_or("buffer view out of bounds")?;
        let view = view.get(a.byte_offset..).ok_or("accessor offset out of bounds")?;
        let stride = v.byte_stride.unwrap_or(len);
        if stride < len || a.count == 0 || (a.count - 1).checked_mul(stride).and_then(|n| n.checked_add(len)).is_none_or(|n| n > view.len()) {
            return Err("accessor out of bounds");
        }
        Ok(view.chunks(stride).take(a.count).map(move |x| &x[..len]))
    };
    let mut scene = GltfScene::default();
    for (i, mesh) in doc.meshes.iter().enumerate() {
        let mut triangles = Vec::new();
        let mut factors = Vec::new();
        for primitive in &mesh.primitives {
            if primitive.mode.unwrap_or(4) != 4 || primitive.targets.is_some() {
                return Err("mesh must use triangles without morph targets".into());
            }
            let pos = *primitive.attributes.get("POSITION").ok_or("missing POSITION")?;
            if doc.accessors.get(pos).is_none_or(|a| a.component_type != 5126) { return Err("POSITION must be float32".into()); }
            let positions = data(pos, 12, "VEC3")?.map(|x| std::array::from_fn(|i| f32::from_le_bytes(x[i * 4..i * 4 + 4].try_into().unwrap())))
                .collect::<Vec<Point>>();
            if !positions.iter().flatten().copied().all(f32::is_finite) { return Err("POSITION includes a nonfinite vertex".into()); }
            let indices = match primitive.indices {
                Some(i) => {
                    let n = match doc.accessors.get(i).ok_or("missing index accessor")?.component_type {
                        5121 => 1, 5123 => 2, 5125 => 4, _ => return Err("index must be unsigned".into()),
                    };
                    data(i, n, "SCALAR")?.map(|x| {
                        let mut word = [0; 4];
                        word[..n].copy_from_slice(x);
                        u32::from_le_bytes(word) as usize
                    }).collect::<Vec<_>>()
                }
                None => (0..positions.len()).collect(),
            };
            if !indices.len().is_multiple_of(3) || indices.iter().any(|&j| j >= positions.len()) {
                return Err("invalid triangle indices".into());
            }
            let default = Material::default();
            let material = primitive.material.map(|id| doc.materials.get(id).ok_or("unknown material")).transpose()?.unwrap_or(&default);
            let pbr = material.pbr_metallic_roughness.as_ref();
            if material.normal_texture.is_some() || material.emissive_texture.is_some()
                || material.emissive_factor.is_some_and(|v| v != [0.; 3])
                || material.alpha_mode.as_ref().is_some_and(|s| s != "OPAQUE")
                || pbr.is_some_and(|p| p.base_color_texture.is_some() || p.metallic_roughness_texture.is_some()) {
                return Err("mesh material requires unsupported textures, emission or blending".into());
            }
            let [r, g, b, _] = pbr.and_then(|p| p.base_color_factor).unwrap_or([1.; 4]);
            let roughness = pbr.and_then(|p| p.roughness_factor).unwrap_or(1.);
            let metal = pbr.and_then(|p| p.metallic_factor).unwrap_or(1.);
            if indices.is_empty() {
                return Err("mesh primitive contains no triangles".into());
            }
            triangles.extend(indices.as_chunks::<3>().0.iter().map(|tri| tri.map(|j| positions[j])));
            factors.push((triangles.len(), Factors { color: [r, g, b], roughness, metal }));
        }
        let asset = MeshAsset(u32::try_from(out.len()).map_err(|_| "too many meshes")?);
        let name = mesh.name.clone().unwrap_or_else(|| i.to_string());
        if scene.meshes.insert(name.clone(), asset).is_some() { return Err(format!("duplicate mesh name {name}")); }
        out.push(Mesh::new(triangles, factors)?);
    }
    Ok(scene)
}

pub(crate) fn lower(src: &crate::source::Graph, modules: &mut [Module]) -> Res<Vec<Mesh>> {
    fn error(span: Span, message: impl Into<String>) -> Diag {
        Diag::new(Code::E602, span, message, "use a supported static glTF triangle mesh")
    }
    fn string(arg: &Arg) -> Res<String> {
        if let ExprKind::String(value) = &arg.value.kind {
            serde_json::from_str(value).map_err(|e| error(arg.value.span, format!("invalid string: {e}")))
        } else {
            Err(error(arg.value.span, "expected a quoted path or mesh name"))
        }
    }
    fn expr(e: &mut Expr, handles: &BTreeMap<String, PathBuf>, scenes: &BTreeMap<PathBuf, GltfScene>) -> Res<()> {
        match &mut e.kind {
            ExprKind::Call { callee, args } if callee.name == ".mesh" => {
                let [receiver, name] = args.as_slice() else { return Err(error(e.span, "mesh takes one name")); };
                let ExprKind::Name(handle) = &receiver.value.kind else { return Err(error(e.span, "bind the import before looking up its meshes")); };
                let path = handles.get(handle).ok_or_else(|| error(e.span, format!("unknown glTF handle {handle}")))?;
                let index = scenes[path].mesh(&string(name)?, name.value.span)?.0;
                e.kind = ExprKind::Num { value: index.into(), suffix: NumSuffix::None, text: index.to_string(), float: false };
            }
            ExprKind::Call { args, .. } | ExprKind::Ctor { args, .. } | ExprKind::FiniteDiff { args, .. } =>
                args.iter_mut().try_for_each(|a| expr(&mut a.value, handles, scenes))?,
            ExprKind::Unary(_, a) | ExprKind::Member(a, _) => expr(a, handles, scenes)?,
            ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) => {
                expr(a, handles, scenes)?;
                expr(b, handles, scenes)?;
            }
            ExprKind::If { .. } | ExprKind::Unproved { .. } | ExprKind::Num { .. } | ExprKind::Bool(_)
            | ExprKind::String(_) | ExprKind::Name(_) => {}
        }
        Ok(())
    }
    fn block(src: &crate::source::Graph, b: &mut Block, mut handles: BTreeMap<String, PathBuf>,
        scenes: &mut BTreeMap<PathBuf, GltfScene>, meshes: &mut Vec<Mesh>) -> Res<()> {
        let mut stmts = Vec::with_capacity(b.stmts.len());
        for mut stmt in std::mem::take(&mut b.stmts) {
            match &mut stmt {
                Stmt::Let { name, mutable: false, ty: None, value: Expr {
                    kind: ExprKind::Call { callee, args }, span, ..
                } } if callee.name == "import_gltf" => {
                    let [arg] = args.as_slice() else { return Err(error(*span, "import_gltf takes one path")); };
                    let source = src.source_path(span.file);
                    let path = relative(source, &string(arg)?).map_err(|e| error(*span, e))?;
                    if !scenes.contains_key(&path) {
                        scenes.insert(path.clone(), load(&path, meshes).map_err(|e| error(*span, e))?);
                    }
                    handles.insert(name.name.clone(), path);
                    continue;
                }
                Stmt::Let { value, name, .. } => {
                    expr(value, &handles, scenes)?;
                    handles.remove(&name.name);
                }
                Stmt::Emit { fields, .. } => fields.iter_mut().try_for_each(|(_, e)| expr(e, &handles, scenes))?,
                Stmt::ImportTable { index, .. } => expr(index, &handles, scenes)?,
                Stmt::For { body, var, .. } | Stmt::Each { body, var, .. } => {
                    let mut inner = handles.clone();
                    inner.remove(&var.name);
                    block(src, body, inner, scenes, meshes)?;
                }
                Stmt::If { then, otherwise, .. } => {
                    block(src, then, handles.clone(), scenes, meshes)?;
                    if let Some(otherwise) = otherwise { block(src, otherwise, handles.clone(), scenes, meshes)?; }
                }
                Stmt::Assign { .. } | Stmt::Map { .. } | Stmt::Return(..) | Stmt::Expr(_) => {}
            }
            stmts.push(stmt);
        }
        b.stmts = stmts;
        Ok(())
    }
    let mut meshes = Vec::new();
    let mut scenes = BTreeMap::new();
    for item in modules.iter_mut().flat_map(|m| &mut m.items) {
        match item {
            Item::Build(f) | Item::Fn(f) if f.comptime => block(src, &mut f.body, BTreeMap::new(), &mut scenes, &mut meshes)?,
            Item::Retarget { name, source, target, .. } => return Err(Diag::new(
                Code::E601, name.span, format!("retarget chain {} to {} is unavailable", source.name, target.name),
                "supply a source clip and a target joint mapping")),
            _ => {}
        }
    }
    Ok(meshes)
}
