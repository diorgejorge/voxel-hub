//! Biblioteca: assets/<id>/asset.json (+ voxels.json se fixo) e scenes/<nome>/scene.json.
use crate::gen::{self, GroundPal};
use crate::pieces;
use crate::voxel::{hex, upscale, Grid, Rgb, S};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Em voxels finos (padrão dos HTMLs): topo do chão em y = 3; objetos apoiam em y = 4.
pub const GY: i32 = 4;
const MISSING: Rgb = 0xff00ff;
pub const MAX_ROUNDS: u32 = 2;
/// Grade da cena em x e z (0..GRID), em voxels de design (GRID × S finos).
/// 128 de design = 256 finos (limite do .vox): o máximo de detalhe que ainda exporta.
pub const GRID: i32 = 128;

#[derive(Serialize, Deserialize, Clone)]
pub struct Asset {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String, // "param" | "fixed"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub piece: Option<String>,
    #[serde(default)]
    pub params: Map<String, Value>,
    #[serde(default)]
    pub palette: BTreeMap<String, String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// "pending" | "approved" | "discarded"
    #[serde(default = "pending")]
    pub status: String,
    #[serde(default)]
    pub rounds: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved: Option<String>,
    /// Campos extras (size, tintable, source, thumb...) são preservados.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn pending() -> String {
    "pending".into()
}

/// Voxels de asset fixo: RLE em ordem x, depois z, depois y. 0 = vazio, n = palette[n-1].
#[derive(Serialize, Deserialize)]
pub struct Rle {
    pub size: [i32; 3],
    pub palette: Vec<String>,
    pub rle: Vec<[u32; 2]>,
}

impl Rle {
    pub fn decode(&self) -> Result<Grid, String> {
        let [sx, sy, sz] = self.size;
        let cols: Vec<Rgb> = self
            .palette
            .iter()
            .map(|h| hex(h).unwrap_or(MISSING))
            .collect();
        let mut g = Grid::default();
        let mut i: i64 = 0;
        for &[count, c] in &self.rle {
            if c > 0 {
                let rgb = *cols
                    .get(c as usize - 1)
                    .ok_or("RLE: índice de cor fora da paleta")?;
                for k in i..i + count as i64 {
                    let k = k as i32;
                    g.put(k % sx - sx / 2, k / (sx * sz), (k / sx) % sz - sz / 2, rgb);
                }
            }
            i += count as i64;
        }
        if i != (sx * sy * sz) as i64 {
            return Err(format!("RLE: {i} células, esperado {}", sx * sy * sz));
        }
        Ok(g)
    }
}

pub fn asset_dir(root: &Path, id: &str) -> PathBuf {
    root.join("assets").join(id)
}

pub fn load_asset(root: &Path, id: &str) -> Result<Asset, String> {
    let a: Asset = read_json(&asset_dir(root, id).join("asset.json"))?;
    // Evita gravar em outro asset quando a pasta foi copiada sem trocar o id.
    if a.id != id {
        return Err(format!(
            "assets/{id}/asset.json tem id \"{}\"; o id deve ser igual ao nome da pasta",
            a.id
        ));
    }
    Ok(a)
}

pub fn save_asset(root: &Path, a: &Asset) -> Result<(), String> {
    let s = serde_json::to_string_pretty(a).map_err(|e| e.to_string())?;
    fs::write(asset_dir(root, &a.id).join("asset.json"), s + "\n").map_err(|e| e.to_string())
}

pub fn list_assets(root: &Path) -> Vec<Asset> {
    let mut v: Vec<Asset> = fs::read_dir(root.join("assets"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| load_asset(root, &e.file_name().to_string_lossy()).ok())
        .collect();
    v.sort_by(|a, b| a.id.cmp(&b.id));
    v
}

/// Gera os voxels do asset, com origem no centro da base.
pub fn asset_grid(
    root: &Path,
    a: &Asset,
    tint: Option<&BTreeMap<String, String>>,
) -> Result<Grid, String> {
    match a.kind.as_str() {
        "param" => {
            let piece = a.piece.as_deref().ok_or("asset param sem 'piece'")?;
            let col = |k: &str| {
                tint.and_then(|t| t.get(k))
                    .or(a.palette.get(k))
                    .and_then(|h| hex(h))
                    .unwrap_or(MISSING)
            };
            pieces::build(piece, &a.params, &col)
        }
        "fixed" => Ok(upscale(
            &read_json::<Rle>(&asset_dir(root, &a.id).join("voxels.json"))?.decode()?,
        )),
        k => Err(format!("tipo de asset desconhecido: {k}")),
    }
}

/// Aplica a decisão da página de aprovação.
pub fn review(
    root: &Path,
    id: &str,
    action: &str,
    comment: Option<String>,
) -> Result<Asset, String> {
    let mut a = load_asset(root, id)?;
    match action {
        "approve" => {
            a.status = "approved".into();
            a.approved = Some(today());
        }
        "adjust" => {
            if a.rounds >= MAX_ROUNDS {
                return Err(format!(
                    "{id}: já teve {MAX_ROUNDS} rodadas de ajuste; aprove ou descarte"
                ));
            }
            a.rounds += 1;
            a.status = "pending".into();
        }
        "discard" => a.status = "discarded".into(),
        _ => return Err(format!("ação inválida: {action}")),
    }
    a.comment = comment.filter(|c| !c.trim().is_empty());
    save_asset(root, &a)?;
    Ok(a)
}

#[derive(Deserialize)]
pub struct Scene {
    #[serde(default)]
    pub ground: Option<Ground>,
    #[serde(default)]
    pub instances: Vec<Instance>,
}

#[derive(Deserialize)]
pub struct Ground {
    /// [x0, x1, z0, z1] inclusivos.
    pub slab: [i32; 4],
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Deserialize)]
pub struct Instance {
    pub asset: String,
    pub at: [i32; 2],
    #[serde(default)]
    pub rot: i32,
    /// Altura extra em voxels de design (0 = apoiado no chão), para empilhar.
    #[serde(default)]
    pub y: i32,
    #[serde(default)]
    pub tint: Option<BTreeMap<String, String>>,
}

pub fn scene_path(root: &Path, name: &str) -> PathBuf {
    root.join("scenes").join(name).join("scene.json")
}

pub fn list_scenes(root: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(root.join("scenes"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| scene_path(root, n).exists())
        .collect();
    v.sort();
    v
}

/// Monta a cena em voxels finos: placa de chão padrão (até y = 3) + instâncias em GY + y.
/// Posições `at`, `y` e `slab` estão em voxels de design. Só usa assets aprovados.
pub fn scene_grid(root: &Path, name: &str) -> Result<Grid, String> {
    let s: Scene = read_json(&scene_path(root, name))?;
    let mut g = Grid::default();
    if let Some(gr) = &s.ground {
        let mut pal = GroundPal::standard();
        if let Some(c) = gr.color.as_deref().and_then(hex) {
            pal = pal.with_grass(c);
        }
        let [x0, x1, z0, z1] = gr.slab;
        gen::ground(
            &mut g,
            &pal,
            [x0 * S, z0 * S],
            [x1 * S + S - 1, z1 * S + S - 1],
            None,
        );
    }
    for i in &s.instances {
        if !(0..GRID).contains(&i.at[0]) || !(0..GRID).contains(&i.at[1]) {
            return Err(format!(
                "{}: posição {:?} fora da grade {GRID}x{GRID}",
                i.asset, i.at
            ));
        }
        if i.rot % 90 != 0 {
            return Err(format!("{}: rot deve ser múltiplo de 90", i.asset));
        }
        let a = load_asset(root, &i.asset)?;
        if a.status != "approved" {
            return Err(format!("{}: asset não aprovado ({})", a.id, a.status));
        }
        let mut one = Grid::default();
        one.stamp(
            &asset_grid(root, &a, i.tint.as_ref())?,
            [i.at[0] * S, GY + i.y * S, i.at[1] * S],
            i.rot,
        );
        // A peça inteira (não só o apoio) tem de caber na grade.
        let (mn, mx) = one.bounds();
        let side = GRID * S;
        if mn[0] < 0 || mn[2] < 0 || mx[0] >= side || mx[2] >= side || mx[1] >= side {
            return Err(format!(
                "{}: em {:?} a peça sai da grade {GRID}x{GRID} (afaste da borda)",
                i.asset, i.at
            ));
        }
        g.vox.extend(one.vox);
    }
    Ok(g)
}

fn read_json<T: for<'de> Deserialize<'de>>(p: &Path) -> Result<T, String> {
    let s = fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
    serde_json::from_str(&s).map_err(|e| format!("{}: {e}", p.display()))
}

/// Data UTC AAAA-MM-DD sem dependências (algoritmo civil de Howard Hinnant).
fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let z = secs / 86400 + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + (m <= 2) as i64;
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rle_decodes_centered() {
        // 2x1x2: duas células cheias, duas vazias.
        let r = Rle {
            size: [2, 1, 2],
            palette: vec!["#ff0000".into()],
            rle: vec![[2, 1], [2, 0]],
        };
        let g = r.decode().unwrap();
        assert_eq!(g.vox.len(), 2);
        assert!(g.solid(-1, 0, -1) && g.solid(0, 0, -1));
    }

    #[test]
    fn rle_rejects_wrong_length() {
        let r = Rle {
            size: [2, 2, 2],
            palette: vec![],
            rle: vec![[3, 0]],
        };
        assert!(r.decode().is_err());
    }

    #[test]
    fn copied_folder_with_other_id_is_rejected() {
        let root = std::env::temp_dir().join(format!("voxel-lib-{}", std::process::id()));
        let dir = asset_dir(&root, "copia");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("asset.json"),
            r#"{"id":"original","type":"param"}"#,
        )
        .unwrap();
        assert!(review(&root, "copia", "discard", None).is_err());
        assert!(!asset_dir(&root, "original").exists());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn today_is_iso_date() {
        let d = today();
        assert_eq!(d.len(), 10);
        assert!(d.starts_with("20"));
    }
}
