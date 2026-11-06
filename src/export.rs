//! Exportação: GLB (Godot/Blender, Y up), VOX (MagicaVoxel), STL (impressão, Z up, mm).
use crate::voxel::{mesh, Grid, Rgb};
use std::collections::HashMap;

pub const FORMATS: [&str; 3] = ["glb", "vox", "stl"];

pub fn export(g: &Grid, format: &str, voxel_size: f32) -> Result<Vec<u8>, String> {
    if g.vox.is_empty() {
        return Err("nada para exportar (grade vazia)".into());
    }
    match format {
        "glb" => Ok(glb(g, voxel_size)),
        "vox" => vox(g),
        "stl" => Ok(stl(g, voxel_size)),
        _ => Err(format!(
            "formato desconhecido: {format} (use {})",
            FORMATS.join(", ")
        )),
    }
}

fn lin(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn bytes<T: Copy>(v: &[T], f: impl Fn(T) -> [u8; 4]) -> Vec<u8> {
    v.iter().flat_map(|&x| f(x)).collect()
}

/// glTF binário. Centro em x/z, base em y = 0. COLOR_0 em espaço linear.
fn glb(g: &Grid, size: f32) -> Vec<u8> {
    let m = mesh(g);
    let (mn, mx) = g.bounds();
    let c = [
        (mn[0] + mx[0] + 1) as f32 / 2.0,
        mn[1] as f32,
        (mn[2] + mx[2] + 1) as f32 / 2.0,
    ];
    let pos: Vec<f32> = m
        .pos
        .iter()
        .enumerate()
        .map(|(i, &v)| (v - c[i % 3]) * size)
        .collect();
    let col: Vec<f32> = m.col.iter().map(|&v| lin(v.min(1.0))).collect();
    let (mut pmin, mut pmax) = ([f32::MAX; 3], [f32::MIN; 3]);
    for (i, &v) in pos.iter().enumerate() {
        pmin[i % 3] = pmin[i % 3].min(v);
        pmax[i % 3] = pmax[i % 3].max(v);
    }
    let views = [
        bytes(&pos, f32::to_le_bytes),
        bytes(&m.nor, f32::to_le_bytes),
        bytes(&col, f32::to_le_bytes),
        bytes(&m.idx, u32::to_le_bytes),
    ];
    let mut off = 0;
    let buffer_views: Vec<_> = views
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let r = serde_json::json!({"buffer": 0, "byteOffset": off, "byteLength": v.len(),
                "target": if i == 3 { 34963 } else { 34962 }});
            off += v.len();
            r
        })
        .collect();
    let n = pos.len() / 3;
    let json = serde_json::json!({
        "asset": {"version": "2.0", "generator": "voxel-hub"},
        "scene": 0, "scenes": [{"nodes": [0]}], "nodes": [{"mesh": 0}],
        "meshes": [{"primitives": [{"attributes": {"POSITION": 0, "NORMAL": 1, "COLOR_0": 2}, "indices": 3, "material": 0}]}],
        "materials": [{"pbrMetallicRoughness": {"baseColorFactor": [1, 1, 1, 1], "metallicFactor": 0, "roughnessFactor": 1}}],
        "accessors": [
            {"bufferView": 0, "componentType": 5126, "count": n, "type": "VEC3", "min": pmin, "max": pmax},
            {"bufferView": 1, "componentType": 5126, "count": n, "type": "VEC3"},
            {"bufferView": 2, "componentType": 5126, "count": n, "type": "VEC3"},
            {"bufferView": 3, "componentType": 5125, "count": m.idx.len(), "type": "SCALAR"}],
        "bufferViews": buffer_views, "buffers": [{"byteLength": off}],
    });
    let mut j = json.to_string().into_bytes();
    j.resize(j.len().next_multiple_of(4), b' ');
    let mut bin = views.concat();
    bin.resize(bin.len().next_multiple_of(4), 0);
    let mut out = Vec::with_capacity(28 + j.len() + bin.len());
    for v in [
        0x46546c67,
        2,
        (28 + j.len() + bin.len()) as u32,
        j.len() as u32,
        0x4e4f534a,
    ] {
        out.extend(u32::to_le_bytes(v));
    }
    out.extend(j);
    out.extend(u32::to_le_bytes(bin.len() as u32));
    out.extend(u32::to_le_bytes(0x004e4942));
    out.extend(bin);
    out
}

/// MagicaVoxel .vox: máx 256 por eixo e 255 cores (z para cima no formato).
fn vox(g: &Grid) -> Result<Vec<u8>, String> {
    let (mn, mx) = g.bounds();
    let dim = [mx[0] - mn[0] + 1, mx[1] - mn[1] + 1, mx[2] - mn[2] + 1];
    if dim.iter().any(|&d| d > 256) {
        return Err("vox: um lado passa de 256 voxels".into());
    }
    let list = g.sorted();
    let mut pal: HashMap<Rgb, u8> = HashMap::new();
    let mut colors: Vec<Rgb> = Vec::new();
    for (_, c) in &list {
        if !pal.contains_key(c) {
            if colors.len() == 255 {
                return Err("vox: mais de 255 cores".into());
            }
            colors.push(*c);
            pal.insert(*c, colors.len() as u8);
        }
    }
    let chunk = |id: &[u8], body: Vec<u8>| -> Vec<u8> {
        let mut v = id.to_vec();
        v.extend(u32::to_le_bytes(body.len() as u32));
        v.extend(u32::to_le_bytes(0));
        v.extend(body);
        v
    };
    let size: Vec<u8> = [dim[0], dim[2], dim[1]]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    let mut xyzi = (list.len() as u32).to_le_bytes().to_vec();
    for (p, c) in &list {
        xyzi.extend([
            (p[0] - mn[0]) as u8,
            (mx[2] - p[2]) as u8,
            (p[1] - mn[1]) as u8,
            pal[c],
        ]);
    }
    let mut rgba = Vec::with_capacity(1024);
    for i in 0..256 {
        let c = colors.get(i).copied().unwrap_or(0);
        rgba.extend([(c >> 16) as u8, (c >> 8) as u8, c as u8, 255]);
    }
    let kids = [
        chunk(b"SIZE", size),
        chunk(b"XYZI", xyzi),
        chunk(b"RGBA", rgba),
    ]
    .concat();
    let mut out = b"VOX ".to_vec();
    out.extend(150u32.to_le_bytes());
    out.extend(b"MAIN");
    out.extend(0u32.to_le_bytes());
    out.extend((kids.len() as u32).to_le_bytes());
    out.extend(kids);
    Ok(out)
}

/// STL binário em mm, Z para cima: (x, y, z) → (x, -z, y), rotação que mantém o sentido das faces.
fn stl(g: &Grid, mm: f32) -> Vec<u8> {
    let m = mesh(g);
    let p = |i: u32| {
        let i = i as usize * 3;
        [m.pos[i] * mm, -m.pos[i + 2] * mm, m.pos[i + 1] * mm]
    };
    let tris = m.idx.len() / 3;
    let mut out = vec![0u8; 80];
    out[..9].copy_from_slice(b"voxel-hub");
    out.extend((tris as u32).to_le_bytes());
    for t in m.idx.chunks(3) {
        let n = t[0] as usize * 3;
        let normal = [m.nor[n], -m.nor[n + 2], m.nor[n + 1]];
        for v in std::iter::once(normal).chain(t.iter().map(|&i| p(i))) {
            v.iter().for_each(|f| out.extend(f.to_le_bytes()));
        }
        out.extend([0, 0]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube() -> Grid {
        let mut g = Grid::default();
        g.fill([0, 0, 0], [1, 1, 1], 0x8040ff);
        g
    }

    #[test]
    fn glb_header_and_length() {
        let b = export(&cube(), "glb", 0.1).unwrap();
        assert_eq!(&b[..4], b"glTF");
        assert_eq!(
            u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize,
            b.len()
        );
        let jl = u32::from_le_bytes(b[12..16].try_into().unwrap()) as usize;
        let j: serde_json::Value = serde_json::from_slice(&b[20..20 + jl]).unwrap();
        assert_eq!(j["accessors"][3]["count"], 24 * 6); // 2x2x2: 24 faces
    }

    #[test]
    fn vox_has_all_voxels() {
        let b = export(&cube(), "vox", 1.0).unwrap();
        assert_eq!(&b[..4], b"VOX ");
        let i = b.windows(4).position(|w| w == b"XYZI").unwrap();
        assert_eq!(u32::from_le_bytes(b[i + 12..i + 16].try_into().unwrap()), 8);
    }

    #[test]
    fn stl_size_matches_triangles() {
        let b = export(&cube(), "stl", 1.0).unwrap();
        let n = u32::from_le_bytes(b[80..84].try_into().unwrap()) as usize;
        assert_eq!(n, 48);
        assert_eq!(b.len(), 84 + n * 50);
    }

    #[test]
    fn empty_grid_errors() {
        assert!(export(&Grid::default(), "glb", 1.0).is_err());
    }
}
