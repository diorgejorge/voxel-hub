//! Geradores portados dos HTMLs do usuário (Carvalho Antigo e Montanha de Pedra),
//! voxel a voxel iguais ao original (sem samambaia, arbusto e juncos). Unidades finas.
use crate::gen::{self, clamp01, disc, hash, hypot, pick, tube, vn, GroundPal};
use crate::voxel::{Grid, Rgb};
use std::collections::HashMap;
use std::f64::consts::PI;

// ---------------------------------------------------------------- carvalho
const LEAF: [Rgb; 7] = [
    0x143520, 0x1d4a2b, 0x2b6030, 0x3b7a36, 0x4f9140, 0x68ab4c, 0x7cbc58,
];
const BARK: [Rgb; 5] = [0x2a1a14, 0x3f271b, 0x56372a, 0x6e4a36, 0x84603f];
const MOSS_OAK: [Rgb; 3] = [0x3d5a24, 0x4a6a2a, 0x5a7a30];
const STONE_OAK: [Rgb; 3] = [0x8e8e88, 0x7a7a78, 0x9a9a94];
const FALL: Rgb = 0xc7742a;
/// Eixo da árvore no HTML (FINE / 2 com SIZE = 48).
pub const OAK_C: i32 = 48;

struct Cluster {
    x: f64,
    y: f64,
    z: f64,
    rx: f64,
    ry: f64,
    rz: f64,
}

fn bark(x: i32, y: i32, z: i32) -> Rgb {
    let s = 0.5 + (hash(x, y, z) - 0.5) * 0.7 + 0.1 * (y as f64 * 0.8 + x as f64).sin();
    BARK[4.min((clamp01(s) * 5.0).floor() as usize)]
}

fn clusters() -> Vec<Cluster> {
    let c = OAK_C as f64;
    let front = PI / 4.0;
    let is_front = |a: f64| (a - front).sin().atan2((a - front).cos()).abs() < 0.6;
    let mut v = vec![Cluster {
        x: c,
        y: 62.0,
        z: c,
        rx: 15.0,
        ry: 9.0,
        rz: 15.0,
    }];
    for i in 0..6 {
        let a = (i as f64 / 6.0) * PI * 2.0 + 0.3;
        let y = 53.0 + ((i % 2) * 2) as f64 + if is_front(a) { 3.0 } else { 0.0 };
        v.push(Cluster {
            x: c + 19.0 * a.cos(),
            y,
            z: c + 19.0 * a.sin(),
            rx: 11.0,
            ry: 8.0,
            rz: 11.0,
        });
    }
    for i in 0..6 {
        let a = (i as f64 / 6.0) * PI * 2.0 + 0.9;
        if is_front(a) {
            continue;
        }
        let y = 43.0 + ((i % 2) * 2) as f64;
        v.push(Cluster {
            x: c + 26.0 * a.cos(),
            y,
            z: c + 26.0 * a.sin(),
            rx: 12.0,
            ry: 7.0,
            rz: 12.0,
        });
    }
    v
}

/// Carvalho antigo em coordenadas finas do HTML (eixo em x = z = 48, chão até y = 3).
pub fn oak(tile: bool) -> Grid {
    let mut g = Grid::default();
    let c = OAK_C as f64;
    if tile {
        gen::ground(
            &mut g,
            &GroundPal::standard(),
            [30, 30],
            [65, 65],
            Some(FALL),
        );
    }
    // tronco
    for y in 4..=36 {
        let yf = y as f64;
        let cx = c + 1.5 * (yf / 4.0).sin();
        let cz = c + 1.5 * (yf / 5.0).cos();
        let r = 7.0 - (yf - 4.0) * 0.12 + 5.0 * (-(yf - 4.0) / 2.2).exp();
        disc(&mut g, cx, cz, r, y, y, |x, yy, z, _| {
            if yy < 11 && hash(x.div_euclid(3), yy >> 1, z.div_euclid(3)) > 0.66 {
                return Some(pick(&MOSS_OAK, x, yy, z));
            }
            let ang = (z as f64 - cz).atan2(x as f64 - cx);
            let s = 0.5
                + 0.32 * (ang * 7.0 + yy as f64 * 0.22).sin()
                + (hash(x, yy, z) - 0.5) * 0.3
                + ang.cos() * 0.12;
            Some(BARK[4.min((clamp01(s) * 5.0).floor() as usize)])
        });
    }
    // raízes
    for a in [0.5f64, 2.0, 3.4, 4.6, 5.6] {
        let s = [c + 4.0 * a.cos(), 7.0, c + 4.0 * a.sin()];
        let e = [c + 14.0 * (a + 0.1).cos(), 4.0, c + 14.0 * (a + 0.1).sin()];
        tube(&mut g, s, e, 3.0, 1.0, &bark);
    }
    let cl = clusters();
    // galhos
    for k in &cl[1..] {
        let e = [c + (k.x - c) * 0.85, k.y - 3.0, c + (k.z - c) * 0.85];
        tube(&mut g, [c, 33.0, c], e, 2.8, 1.3, &bark);
    }
    // copa: elipsoides com borda irregular; cada voxel fica com o cluster mais próximo
    let mut best: HashMap<[i32; 3], (f64, usize)> = HashMap::new();
    for (i, k) in cl.iter().enumerate() {
        let r = |c: f64, rr: f64| (c - rr * 1.3).floor() as i32..=(c + rr * 1.3).ceil() as i32;
        for x in r(k.x, k.rx) {
            for y in r(k.y, k.ry) {
                for z in r(k.z, k.rz) {
                    let dx = (x as f64 + 0.5 - k.x) / k.rx;
                    let dy = (y as f64 + 0.5 - k.y) / k.ry;
                    let dz = (z as f64 + 0.5 - k.z) / k.rz;
                    let e = dx * dx + dy * dy + dz * dz;
                    if e >= 1.0 + 0.5 * (hash(x >> 1, y >> 1, z >> 1) - 0.5) {
                        continue;
                    }
                    let slot = best.entry([x, y, z]).or_insert((f64::INFINITY, i));
                    if e < slot.0 {
                        *slot = (e, i);
                    }
                }
            }
        }
    }
    for ([x, y, z], (e, i)) in best {
        let k = &cl[i];
        let ny = (y as f64 + 0.5 - k.y) / k.ry;
        let s = 0.3
            + 0.36 * ny
            + (y as f64 - 45.0) / 110.0
            + (hash(x, y, z) - 0.5) * 0.34
            + if e > 0.6 { 0.05 } else { 0.0 };
        g.put(x, y, z, LEAF[6.min((clamp01(s) * 7.0).floor() as usize)]);
    }
    for (x, z, r, h) in [
        (37.0, 52.0, 2.4, 2.6),
        (47.0, 60.0, 3.0, 2.4),
        (55.0, 62.0, 1.4, 1.2),
    ] {
        gen::stone(&mut g, &STONE_OAK, x, z, r, h);
    }
    g
}

// ---------------------------------------------------------------- montanha
const MOSS_CRAG: [Rgb; 3] = [0x4a6a2a, 0x5a7a30, 0x3e5a26];
const DIRT: [Rgb; 3] = [0x6e5f54, 0x5e5047, 0x7a6a5c];
const PEBBLE: [Rgb; 4] = [0x9a9a94, 0x8a8a86, 0x7a7a78, 0xb0b0aa];
const CRACK: Rgb = 0x2d3140;
const CORE: Rgb = 0x454b5a;
const RAMP: [Rgb; 8] = [
    0x2d3140, 0x3d4252, 0x4b5063, 0x6a6e7e, 0x7a8ba0, 0x8f8a7c, 0xa89f8c, 0xbdb39a,
];
const WARM: [Rgb; 5] = [0x54484a, 0x6a5e56, 0x7a6c60, 0x8a7c6c, 0xa09280];
/// Lado da placa da montanha no HTML (SIZE = 32 → 64 finos).
pub const CRAG_FINE: i32 = 64;
const CELL: f64 = 7.0;

/// Uma rocha da pilha (unidades finas): perfil cônico (`e`) ou domo; `lit` clareia a face.
pub struct Rock {
    pub x: f64,
    pub z: f64,
    pub r: f64,
    pub h: f64,
    pub e: f64,
    pub dome: bool,
    pub lit: bool,
}

/// As 7 rochas empilhadas da montanha do HTML.
pub const BLOBS: [Rock; 7] = [
    Rock {
        x: 30.0,
        z: 28.0,
        r: 24.0,
        h: 38.0,
        e: 0.95,
        dome: false,
        lit: false,
    },
    Rock {
        x: 16.0,
        z: 38.0,
        r: 13.0,
        h: 17.0,
        e: 0.8,
        dome: false,
        lit: false,
    },
    Rock {
        x: 44.0,
        z: 17.0,
        r: 12.0,
        h: 24.0,
        e: 0.8,
        dome: false,
        lit: false,
    },
    Rock {
        x: 52.0,
        z: 25.0,
        r: 9.0,
        h: 15.0,
        e: 0.8,
        dome: false,
        lit: false,
    },
    Rock {
        x: 50.0,
        z: 34.0,
        r: 8.0,
        h: 11.0,
        e: 0.8,
        dome: false,
        lit: false,
    },
    Rock {
        x: 47.0,
        z: 46.0,
        r: 11.0,
        h: 13.0,
        e: 0.0,
        dome: true,
        lit: true,
    },
    Rock {
        x: 21.0,
        z: 50.0,
        r: 8.0,
        h: 9.0,
        e: 0.0,
        dome: true,
        lit: false,
    },
];

/// Voronoi 3D em grade com jitter: (distância até a borda da célula, id da célula).
fn voronoi(x: i32, y: i32, z: i32) -> (f64, f64) {
    let c = [x, y, z].map(|v| (v as f64 / CELL).floor() as i32);
    let (mut d1, mut d2, mut id) = (1e9f64, 1e9f64, 0.0);
    for dx in -1..=1 {
        for dy in -1..=1 {
            for dz in -1..=1 {
                let (gx, gy, gz) = (c[0] + dx, c[1] + dy, c[2] + dz);
                let px = (gx as f64 + hash(gx, gy, gz)) * CELL;
                let py = (gy as f64 + hash(gx, gy + 9, gz)) * CELL;
                let pz = (gz as f64 + hash(gx + 5, gy, gz)) * CELL;
                let d = hypot(&[
                    x as f64 + 0.5 - px,
                    y as f64 + 0.5 - py,
                    z as f64 + 0.5 - pz,
                ]);
                if d < d1 {
                    d2 = d1;
                    d1 = d;
                    id = hash(gx, gy + 3, gz);
                } else if d < d2 {
                    d2 = d;
                }
            }
        }
    }
    (d2 - d1, id)
}

fn rock_height(rocks: &[Rock], x: i32, z: i32) -> i32 {
    let mut best = 0.0f64;
    for b in rocks {
        let d = hypot(&[x as f64 + 0.5 - b.x, z as f64 + 0.5 - b.z]) / b.r;
        if d >= 1.0 {
            continue;
        }
        let prof = if b.dome {
            (1.0 - d * d).sqrt().powf(0.9)
        } else {
            (1.0 - d).powf(b.e)
        };
        best = best.max(b.h * prof);
    }
    if best <= 0.0 {
        return 0;
    }
    let (xf, zf) = (x as f64, z as f64);
    let crag = 2.2 * vn(xf / 5.0, zf / 5.0, 1) + 1.2 * vn(xf / 2.4, zf / 2.4, 2) - 1.7;
    (best + crag * (best / 5.0).min(1.0)).floor().max(0.0) as i32
}

fn dirt_at(x: i32, z: i32) -> bool {
    let (x, z) = (x as f64, z as f64);
    let n = (vn(x / 3.0, z / 3.0, 6) - 0.5) * 0.35;
    hypot(&[(x - 34.0) / 11.0, (z - 54.0) / 6.5]) + n < 1.0
        || hypot(&[(x - 18.0) / 6.0, (z - 52.0) / 4.0]) + n < 1.0
        || hypot(&[(x - 31.0) / 4.0, (z - 45.0) / 6.0]) + n < 1.0
}

/// Montanha de pedra em coordenadas finas do HTML (placa 0..64, chão até y = 3).
/// Montanha do HTML: as 7 rochas empilhadas + terra, seixos e pedras soltas.
pub fn crag(tile: bool) -> Grid {
    let mut g = rockpile(&BLOBS, CRAG_FINE, tile, &dirt_at);
    for (x, z, r, h) in STONES {
        gen::stone(&mut g, &PEBBLE, x, z, r, h);
    }
    g
}

const STONES: [(f64, f64, f64, f64); 8] = [
    (7.0, 38.0, 3.6, 4.0),
    (10.0, 45.0, 1.4, 2.0),
    (38.0, 60.0, 3.6, 4.0),
    (52.0, 60.0, 2.4, 3.0),
    (58.0, 30.0, 3.0, 3.0),
    (56.0, 40.0, 1.5, 2.0),
    (30.0, 57.0, 1.6, 2.0),
    (20.0, 59.0, 1.2, 1.5),
];

/// Pilha de rochas numa placa n×n fina: cada coluna pega a rocha mais alta, e a cor
/// (rampa, rachaduras voronoi, musgo) vem da superfície da pilha inteira.
/// `dirt` marca manchas de terra na placa.
pub fn rockpile(rocks: &[Rock], n: i32, tile: bool, dirt: &dyn Fn(i32, i32) -> bool) -> Grid {
    let heights: Vec<i32> = (0..n * n)
        .map(|i| rock_height(rocks, i / n, i % n))
        .collect();
    let h_at = |x: i32, z: i32| {
        if x < 0 || z < 0 || x >= n || z >= n {
            0
        } else {
            heights[(x * n + z) as usize]
        }
    };
    let mut g = Grid::default();
    if tile {
        let mut p = GroundPal::standard();
        p.grass = vec![0x3a5a30, 0x41643a, 0x35502c, 0x456a3c];
        p.tuft = vec![0x5b8a40, 0x4a7234, 0x6a9a48];
        p.soil = vec![0x6e4129, 0x78472d, 0x633a25];
        p.soil_deep = 0x3c2318;
        p.soil_top = 0x4d2f1e;
        for x in 0..n {
            for z in 0..n {
                let edge = x < 2 || z < 2 || x >= n - 2 || z >= n - 2;
                gen::tile_base(&mut g, &p, x, z, [-6, -3], edge);
                let dirt = dirt(x, z);
                let top = if dirt {
                    pick(&DIRT, x, 3, z)
                } else if edge {
                    p.rim
                } else {
                    pick(&p.grass, x, 3, z)
                };
                g.put(x, 3, z, top);
                if edge || h_at(x, z) != 0 {
                    continue;
                }
                let r = hash(x, 9, z);
                if dirt {
                    if r > 0.985 {
                        g.fill([x, 4, z], [x + 1, 4, z + 1], pick(&PEBBLE, x, 4, z));
                    } else if r > 0.96 {
                        g.put(x, 4, z, pick(&PEBBLE, x, 4, z));
                    }
                } else if r > 0.94 {
                    g.put(x, 4, z, pick(&p.tuft, x, 4, z));
                    if r > 0.985 {
                        g.put(x, 5, z, pick(&p.tuft, x, 5, z));
                    }
                }
            }
        }
    }
    let rock_color = |x: i32, y: i32, z: i32| -> Rgb {
        let gx = (h_at(x + 2, z) - h_at(x - 2, z)) as f64 / 4.0;
        let gz = (h_at(x, z + 2) - h_at(x, z - 2)) as f64 / 4.0;
        let len = hypot(&[gx, 1.0, gz]);
        let mut s = -0.1 * (-gx / len) + 0.9 / len + 0.35 * (-gz / len) - 0.15;
        let (edge, id) = voronoi(x, y, z);
        if edge < 0.55 {
            return CRACK;
        }
        s += (id - 0.5) * 0.34 + ((y - 4) as f64 / 40.0) * 0.12 + 0.04;
        if rocks
            .iter()
            .any(|b| b.lit && hypot(&[x as f64 - b.x, z as f64 - b.z]) < b.r)
        {
            s += 0.12;
        }
        if y < 14 && s < 0.62 && ((edge < 2.4 && id > 0.3) || (id > 0.6 && hash(x, y, z) > 0.45)) {
            return pick(&MOSS_CRAG, x, y, z);
        }
        if id > 0.74 {
            return WARM[4.min((clamp01(s) * 6.0).floor() as usize)];
        }
        RAMP[7.min((clamp01(s) * 8.0).floor() as usize)]
    };
    for x in 0..n {
        for z in 0..n {
            let h = h_at(x, z);
            if h == 0 {
                continue;
            }
            let top = 3 + h;
            let lowest = [
                h_at(x + 1, z),
                h_at(x - 1, z),
                h_at(x, z + 1),
                h_at(x, z - 1),
            ]
            .into_iter()
            .min()
            .unwrap()
                + 3;
            for y in 4..=top {
                g.put(
                    x,
                    y,
                    z,
                    if y == top || y > lowest {
                        rock_color(x, y, z)
                    } else {
                        CORE
                    },
                );
            }
        }
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FNV-1a sobre a lista ordenada (y, z, x, cor): impressão digital da grade.
    fn fingerprint(g: &Grid) -> (usize, u64) {
        let mut h: u64 = 0xcbf29ce484222325;
        for (p, c) in g.sorted() {
            for v in [p[0], p[1], p[2], c as i32] {
                for b in v.to_le_bytes() {
                    h ^= b as u64;
                    h = h.wrapping_mul(0x100000001b3);
                }
            }
        }
        (g.vox.len(), h)
    }

    // Referência gerada pelo HTML original (tools/compare-html.mjs), sem samambaia/arbusto/juncos.
    #[test]
    fn oak_matches_html() {
        assert_eq!(fingerprint(&oak(true)), OAK_REF);
    }

    #[test]
    fn crag_matches_html() {
        assert_eq!(fingerprint(&crag(true)), CRAG_REF);
    }

    const OAK_REF: (usize, u64) = (66498, 12361543666037308865);
    const CRAG_REF: (usize, u64) = (71031, 17626506330278060156);
}
