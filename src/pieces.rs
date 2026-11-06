//! Peças paramétricas. Origem = centro da base (x, z), y = 0. Medidas em voxels de design.
use crate::voxel::{Grid, Rgb};
use serde_json::{Map, Value};

/// Resolve um nome de cor da paleta do asset (com tint já aplicado).
pub type Colors<'a> = dyn Fn(&str) -> Rgb + 'a;

pub const NAMES: [&str; 4] = ["box", "cyl", "ball", "barrel"];

pub fn build(piece: &str, p: &Map<String, Value>, col: &Colors) -> Result<Grid, String> {
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
        assert_eq!(g.bounds(), ([-2, 0, -1], [1, 2, 0]));
        assert_eq!(g.vox.len(), 24);
    }

    #[test]
    fn unknown_piece_errors() {
        assert!(build("nope", &Map::new(), &|_| 1).is_err());
    }
}
