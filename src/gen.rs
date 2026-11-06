//! Base comum dos geradores (porte fiel do engine HTML): hash, paleta em rampa,
//! formas (disc, ball, tube, stone), ruído e o chão em placa. Tudo em unidades finas
//! (S = 2 por voxel de design), como no HTML.
use crate::voxel::{Grid, Rgb};

/// Hash inteiro → [0, 1). Igual ao `hash` do JS (Math.imul e >>> em 32 bits).
pub fn hash(x: i32, y: i32, z: i32) -> f64 {
    let w = |v: i32, k: i64| (v as i64 * k) as i32;
    let mut h =
        (w(x, 73856093) ^ w(y, 19349663) ^ w(z, 83492791)).wrapping_mul(0x85ebca6bu32 as i32);
    h ^= ((h as u32) >> 13) as i32;
    h = h.wrapping_mul(0xc2b2ae35u32 as i32);
    h ^= ((h as u32) >> 16) as i32;
    (h as u32) as f64 / 4294967296.0
}

pub fn pick(list: &[Rgb], x: i32, y: i32, z: i32) -> Rgb {
    list[(hash(x, y, z) * list.len() as f64).floor() as usize]
}

/// Escurece/clareia por canal (Math.round do JS: meio arredonda para cima).
pub fn scale(hex: Rgb, f: f64) -> Rgb {
    let ch = |s: u32| (js_round(((hex >> s) & 255) as f64 * f) as i64).clamp(0, 255) as u32;
    (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

pub fn js_round(v: f64) -> f64 {
    (v + 0.5).floor()
}

pub fn clamp01(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

/// Math.hypot do V8 (escala pelo maior e soma compensada), para bater bit a bit com o HTML.
pub fn hypot(v: &[f64]) -> f64 {
    let max = v.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    if max == 0.0 {
        return 0.0;
    }
    let (mut sum, mut comp) = (0.0f64, 0.0f64);
    for x in v {
        let n = x.abs() / max;
        let summand = n * n - comp;
        let pre = sum + summand;
        comp = (pre - sum) - summand;
        sum = pre;
    }
    sum.sqrt() * max
}

/// Coluna de discos: cor por voxel (None = vazio). Igual ao `disc` do HTML.
pub fn disc(
    g: &mut Grid,
    cx: f64,
    cz: f64,
    radius: f64,
    y0: i32,
    y1: i32,
    color: impl Fn(i32, i32, i32, f64) -> Option<Rgb>,
) {
    let r = radius.ceil() as i32;
    for x in cx.floor() as i32 - r..=cx.ceil() as i32 + r {
        for z in cz.floor() as i32 - r..=cz.ceil() as i32 + r {
            let d = hypot(&[x as f64 + 0.5 - cx, z as f64 + 0.5 - cz]);
            if d > radius {
                continue;
            }
            for y in y0..=y1 {
                if let Some(c) = color(x, y, z, d) {
                    g.put(x, y, z, c);
                }
            }
        }
    }
}

pub fn ball(g: &mut Grid, c: [f64; 3], r: f64, color: &impl Fn(i32, i32, i32) -> Rgb) {
    let n = r.ceil() as i32;
    let o = c.map(|v| js_round(v) as i32);
    for dx in -n..=n {
        for dy in -n..=n {
            for dz in -n..=n {
                if hypot(&[dx as f64, dy as f64, dz as f64]) <= r {
                    let (x, y, z) = (o[0] + dx, o[1] + dy, o[2] + dz);
                    g.put(x, y, z, color(x, y, z));
                }
            }
        }
    }
}

/// Tubo cônico de a até b (bolas ao longo do segmento).
pub fn tube(
    g: &mut Grid,
    a: [f64; 3],
    b: [f64; 3],
    r0: f64,
    r1: f64,
    color: &impl Fn(i32, i32, i32) -> Rgb,
) {
    let len = hypot(&[b[0] - a[0], b[1] - a[1], b[2] - a[2]]);
    let steps = (len * 2.0).ceil() as i32;
    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let p = [0, 1, 2].map(|k| a[k] + (b[k] - a[k]) * t);
        ball(g, p, r0 + (r1 - r0) * t, color);
    }
}

/// Pedra em domo apoiada em y = 4.
pub fn stone(g: &mut Grid, pal: &[Rgb], cx: f64, cz: f64, radius: f64, height: f64) {
    disc(
        g,
        cx,
        cz,
        radius,
        4,
        4 + height.ceil() as i32,
        |x, y, z, d| {
            let q = d / radius;
            let dome = height * (1.0 - q * q).max(0.0).sqrt();
            (((y - 4) as f64) < dome + 0.5).then(|| {
                scale(
                    pick(pal, x, y, z),
                    0.85 + 0.25 * (y - 4) as f64 / height.max(1.0),
                )
            })
        },
    );
}

/// Ruído de valor suave em grade.
pub fn vn(x: f64, z: f64, seed: i32) -> f64 {
    let (xi, zi) = (x.floor(), z.floor());
    let (fx, fz) = (x - xi, z - zi);
    let (xi, zi) = (xi as i32, zi as i32);
    let a = hash(xi, seed, zi);
    let b = hash(xi + 1, seed, zi);
    let c = hash(xi, seed, zi + 1);
    let d = hash(xi + 1, seed, zi + 1);
    let u = fx * fx * (3.0 - 2.0 * fx);
    let v = fz * fz * (3.0 - 2.0 * fz);
    a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v
}

/// Paleta padrão do chão (rampas com variação por hash).
pub struct GroundPal {
    pub grass: Vec<Rgb>,
    pub tuft: Vec<Rgb>,
    pub soil: Vec<Rgb>,
    pub soil_deep: Rgb,
    pub soil_top: Rgb,
    pub rim: Rgb,
}

impl GroundPal {
    pub fn standard() -> Self {
        GroundPal {
            grass: vec![0x35602f, 0x3d6a35, 0x2f5429, 0x436f38],
            tuft: vec![0x4f8a3a, 0x3f7030, 0x62a048],
            soil: vec![0x5a3a28, 0x4e3022, 0x65402c],
            soil_deep: 0x2a1a1c,
            soil_top: 0x3a2418,
            rim: 0x2f4a28,
        }
    }

    /// Grama derivada de uma cor base (variações claras/escuras).
    pub fn with_grass(mut self, base: Rgb) -> Self {
        self.grass = vec![base, scale(base, 1.1), scale(base, 0.88), scale(base, 1.18)];
        self
    }
}

/// Camadas da placa de chão (fundo até y = 2). Grama (y = 3) e enfeites ficam com quem chama.
/// `deep`: y inicial da terra escura; a terra comum vai de `deep + depth` até 1.
pub fn tile_base(g: &mut Grid, p: &GroundPal, x: i32, z: i32, deep: [i32; 2], edge: bool) {
    for y in deep[0]..=deep[1] {
        g.put(x, y, z, p.soil_deep);
    }
    for y in deep[1] + 1..=1 {
        g.put(x, y, z, pick(&p.soil, x, y, z));
    }
    g.put(x, 2, z, if edge { p.rim } else { p.soil_top });
}

/// Placa de chão padrão com tufos (a mesma do carvalho). Faixa fina [lo, hi] em x e z.
pub fn ground(g: &mut Grid, p: &GroundPal, lo: [i32; 2], hi: [i32; 2], fall: Option<Rgb>) {
    for x in lo[0]..=hi[0] {
        for z in lo[1]..=hi[1] {
            let edge = x < lo[0] + 1 || z < lo[1] + 1 || x > hi[0] - 1 || z > hi[1] - 1;
            tile_base(g, p, x, z, [-3, -2], edge);
            g.put(x, 3, z, pick(&p.grass, x, 3, z));
            let r = hash(x, 9, z);
            if edge {
                continue;
            }
            if r > 0.93 {
                g.put(x, 4, z, pick(&p.tuft, x, 4, z));
            } else if let Some(f) = fall.filter(|_| r < 0.006) {
                g.put(x, 4, z, f);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_matches_js() {
        // Valores calculados com o hash do HTML no Node.
        assert_eq!(hash(0, 0, 0), 0.0);
        assert_eq!(hash(1, 2, 3), 0.2587974132038653);
        assert_eq!(hash(-5, 9, 40), 0.07887080288492143);
    }

    #[test]
    fn hypot_basic() {
        assert_eq!(hypot(&[3.0, 4.0]), 5.0);
        assert_eq!(hypot(&[0.0, 0.0]), 0.0);
    }
}
