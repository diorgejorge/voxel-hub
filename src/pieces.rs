//! Peças paramétricas. Origem = centro da base (x, z), y = 0. Medidas em voxels de design.
use crate::organic;
use crate::voxel::{upscale, Grid, Rgb, S};
use serde_json::{Map, Value};

/// Resolve um nome de cor da paleta do asset (com tint já aplicado).
pub type Colors<'a> = dyn Fn(&str) -> Rgb + 'a;

pub const NAMES: [&str; 7] = ["box", "cyl", "ball", "barrel", "oak", "crag", "rockpile"];

/// Gera a peça em voxels finos (S por voxel de design), origem no centro da base.
/// `oak` e `crag` são os geradores dos HTMLs (já finos); `tile: 0` tira a placa de chão.
/// `rockpile`: rochas empilhadas como a montanha (`crag` é uma pilha pronta).
pub fn build(piece: &str, p: &Map<String, Value>, col: &Colors) -> Result<Grid, String> {
    let tile = p.get("tile").and_then(Value::as_f64).unwrap_or(1.0) != 0.0;
    match piece {
        "oak" => Ok(recenter(&organic::oak(tile), organic::OAK_C)),
        "crag" => Ok(recenter(&organic::crag(tile), organic::CRAG_FINE / 2)),
        "rockpile" => rockpile(p, tile),
        _ => Ok(upscale(&build_design(piece, p, col)?)),
    }
}

/// `rocks: [{x, z, r, h, e?, dome?, lit?}]` em voxels de design dentro de `size`×`size` (padrão 32).
fn rockpile(p: &Map<String, Value>, tile: bool) -> Result<Grid, String> {
    let size = p.get("size").and_then(Value::as_f64).unwrap_or(32.0) as i32;
    let list = p
        .get("rocks")
        .and_then(Value::as_array)
        .filter(|l| !l.is_empty())
        .ok_or("rockpile: falta a lista rocks")?;
    let mut rocks = Vec::new();
    for v in list {
        let f = |k: &str, d: f64| v.get(k).and_then(Value::as_f64).unwrap_or(d);
        let on = |k: &str| v.get(k).and_then(Value::as_f64).unwrap_or(0.0) != 0.0;
        let s = S as f64;
        rocks.push(organic::Rock {
            x: f("x", size as f64 / 2.0) * s,
            z: f("z", size as f64 / 2.0) * s,
            r: f("r", 4.0) * s,
            h: f("h", 4.0) * s,
            e: f("e", 0.8),
            dome: on("dome"),
            lit: on("lit"),
        });
    }
    let n = size * S;
    Ok(recenter(
        &organic::rockpile(&rocks, n, tile, &|_, _| false),
        n / 2,
    ))
}

/// Coordenadas do HTML → origem no centro da base (objetos do HTML apoiam em y = 4).
fn recenter(g: &Grid, c: i32) -> Grid {
    let mut out = Grid::default();
    for (&[x, y, z], &v) in &g.vox {
        out.put(x - c, y - 4, z - c, v);
    }
    out
}

/// Peças simples em voxels de design.
fn build_design(piece: &str, p: &Map<String, Value>, col: &Colors) -> Result<Grid, String> {
    let n = |k: &str, d: f64| p.get(k).and_then(Value::as_f64).unwrap_or(d);
    let mut g = Grid::default();
    match piece {
        "box" => {
            let (w, h, d) = (n("w", 4.0) as i32, n("h", 4.0) as i32, n("d", 4.0) as i32);
            g.fill(
                [-(w / 2), 0, -(d / 2)],
                [w - w / 2 - 1, h - 1, d - d / 2 - 1],
                col("box"),
            );
        }
        "cyl" => disc_stack(
            &mut g,
            n("h", 4.0) as i32,
            |_| n("r", 2.0),
            |_, _, _| col("cyl"),
        ),
        "ball" => {
            let r = n("r", 3.0);
            let ri = r.ceil() as i32;
            for y in 0..2 * ri {
                for z in -ri..ri {
                    for x in -ri..ri {
                        let (dx, dy, dz) = (x as f64 + 0.5, y as f64 + 0.5 - r, z as f64 + 0.5);
                        if dx * dx + dy * dy + dz * dz <= r * r {
                            g.put(x, y, z, col("ball"));
                        }
                    }
                }
            }
        }
        "barrel" => {
            let (r, h) = (n("r", 4.0), n("h", 8.0) as i32);
            let hoops = [1, h - 2];
            disc_stack(
                &mut g,
                h,
                |y| r * (0.85 + 0.15 * (std::f64::consts::PI * (y as f64 + 0.5) / h as f64).sin()),
                |x, y, z| {
                    if hoops.contains(&y) {
                        return col("hoop");
                    }
                    // Aduelas: alterna cor por fatia de ângulo.
                    let a = (z as f64 + 0.5).atan2(x as f64 + 0.5);
                    let k = ((a + std::f64::consts::PI) / (std::f64::consts::PI / 6.0)) as i32;
                    col(if k % 2 == 0 { "barrel" } else { "barrelD" })
                },
            );
        }
        _ => {
            return Err(format!(
                "peça desconhecida: {piece} (existem: {})",
                NAMES.join(", ")
            ))
        }
    }
    Ok(g)
}

/// Pilha de discos: raio por altura e cor por voxel.
fn disc_stack(
    g: &mut Grid,
    h: i32,
    radius: impl Fn(i32) -> f64,
    color: impl Fn(i32, i32, i32) -> Rgb,
) {
    for y in 0..h {
        let r = radius(y);
        let ri = r.ceil() as i32;
        for z in -ri..ri {
            for x in -ri..ri {
                let (dx, dz) = (x as f64 + 0.5, z as f64 + 0.5);
                if dx * dx + dz * dz <= r * r {
                    g.put(x, y, z, color(x, y, z));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_is_centered_on_base() {
        let p = serde_json::json!({"w": 4, "h": 3, "d": 2});
        let g = build("box", p.as_object().unwrap(), &|_| 1).unwrap();
        // 4x3x2 de design → 8x6x4 finos, centrado na base.
        assert_eq!(g.bounds(), ([-4, 0, -2], [3, 5, 1]));
        assert_eq!(g.vox.len(), 24 * 8);
    }

    #[test]
    fn rockpile_of_crag_rocks_is_the_crag_mountain() {
        // As 7 rochas da montanha (fino / 2) dão a mesma pilha do `crag` (sem placa nem pedras soltas).
        let rocks = serde_json::json!({"tile": 0, "rocks": [
            {"x": 15, "z": 14, "r": 12, "h": 19, "e": 0.95},
            {"x": 8, "z": 19, "r": 6.5, "h": 8.5},
            {"x": 22, "z": 8.5, "r": 6, "h": 12},
            {"x": 26, "z": 12.5, "r": 4.5, "h": 7.5},
            {"x": 25, "z": 17, "r": 4, "h": 5.5},
            {"x": 23.5, "z": 23, "r": 5.5, "h": 6.5, "dome": 1, "lit": 1},
            {"x": 10.5, "z": 25, "r": 4, "h": 4.5, "dome": 1}]});
        let pile = build("rockpile", rocks.as_object().unwrap(), &|_| 1).unwrap();
        let crag = organic::rockpile(&organic::BLOBS, organic::CRAG_FINE, false, &|_, _| false);
        assert_eq!(pile.vox, recenter(&crag, organic::CRAG_FINE / 2).vox);
    }

    #[test]
    fn rockpile_needs_rocks() {
        assert!(build("rockpile", &Map::new(), &|_| 1).is_err());
    }

    #[test]
    fn unknown_piece_errors() {
        assert!(build("nope", &Map::new(), &|_| 1).is_err());
    }
}
