//! Grade de voxels esparsa e malha com oclusão ambiente (AO) por vértice.
use std::collections::HashMap;

/// Cor RGB 0xRRGGBB.
pub type Rgb = u32;

#[derive(Default, Clone)]
pub struct Grid {
    pub vox: HashMap<[i32; 3], Rgb>,
}

impl Grid {
    pub fn put(&mut self, x: i32, y: i32, z: i32, c: Rgb) {
        self.vox.insert([x, y, z], c);
    }

    pub fn solid(&self, x: i32, y: i32, z: i32) -> bool {
        self.vox.contains_key(&[x, y, z])
    }

    /// Preenche a caixa [a, b] inclusiva.
    pub fn fill(&mut self, a: [i32; 3], b: [i32; 3], c: Rgb) {
        for y in a[1]..=b[1] {
            for z in a[2]..=b[2] {
                for x in a[0]..=b[0] {
                    self.put(x, y, z, c);
                }
            }
        }
    }

    /// Copia `src` girado `rot` graus (múltiplo de 90) no eixo Y e deslocado por `at`.
    pub fn stamp(&mut self, src: &Grid, at: [i32; 3], rot: i32) {
        for (&p, &c) in &src.vox {
            let [x, z] = rot90(p[0], p[2], rot);
            self.put(x + at[0], p[1] + at[1], z + at[2], c);
        }
    }

    /// (min, max) inclusivos. Grade vazia retorna ([0;3], [-1;3]).
    pub fn bounds(&self) -> ([i32; 3], [i32; 3]) {
        let mut mn = [i32::MAX; 3];
        let mut mx = [i32::MIN; 3];
        for p in self.vox.keys() {
            for i in 0..3 {
                mn[i] = mn[i].min(p[i]);
                mx[i] = mx[i].max(p[i]);
            }
        }
        if self.vox.is_empty() {
            return ([0; 3], [-1; 3]);
        }
        (mn, mx)
    }

    /// Voxels em ordem estável (y, z, x).
    pub fn sorted(&self) -> Vec<([i32; 3], Rgb)> {
        let mut v: Vec<_> = self.vox.iter().map(|(p, c)| (*p, *c)).collect();
        v.sort_by_key(|(p, _)| (p[1], p[2], p[0]));
        v
    }
}

/// Gira a célula (x, z) em passos de 90° em torno da origem (centro da base).
/// A célula x cobre [x, x+1], então o giro usa o centro x+0.5.
pub fn rot90(x: i32, z: i32, rot: i32) -> [i32; 2] {
    match rot.rem_euclid(360) / 90 {
        1 => [-z - 1, x],
        2 => [-x - 1, -z - 1],
        3 => [z, -x - 1],
        _ => [x, z],
    }
}

pub fn hex(s: &str) -> Option<Rgb> {
    u32::from_str_radix(s.trim_start_matches('#'), 16).ok()
}

/// Malha triangulada: posições em unidades de voxel, cores sRGB 0..1 com AO.
#[derive(Default)]
pub struct Mesh {
    pub pos: Vec<f32>,
    pub nor: Vec<f32>,
    pub col: Vec<f32>,
    pub idx: Vec<u32>,
}

const FACES: [([i32; 3], [[i32; 3]; 4]); 6] = [
    ([1, 0, 0], [[1, 0, 1], [1, 0, 0], [1, 1, 0], [1, 1, 1]]),
    ([-1, 0, 0], [[0, 0, 0], [0, 0, 1], [0, 1, 1], [0, 1, 0]]),
    ([0, 1, 0], [[0, 1, 1], [1, 1, 1], [1, 1, 0], [0, 1, 0]]),
    ([0, -1, 0], [[0, 0, 0], [1, 0, 0], [1, 0, 1], [0, 0, 1]]),
    ([0, 0, 1], [[0, 0, 1], [1, 0, 1], [1, 1, 1], [0, 1, 1]]),
    ([0, 0, -1], [[1, 0, 0], [0, 0, 0], [0, 1, 0], [1, 1, 0]]),
];
const AO_LEVEL: [f32; 4] = [0.58, 0.74, 0.88, 1.0];

fn corner_ao(g: &Grid, p: [i32; 3], n: [i32; 3], corner: [i32; 3]) -> usize {
    // Dois eixos tangentes à face, apontando para o canto.
    let mut t = [[0; 3]; 2];
    let mut k = 0;
    for a in 0..3 {
        if n[a] == 0 {
            t[k][a] = if corner[a] == 1 { 1 } else { -1 };
            k += 1;
        }
    }
    let b = [p[0] + n[0], p[1] + n[1], p[2] + n[2]];
    let s = |d: [i32; 3]| g.solid(b[0] + d[0], b[1] + d[1], b[2] + d[2]) as usize;
    let (u, v) = (t[0], t[1]);
    let s1 = s(u);
    let s2 = s(v);
    let dg = s([u[0] + v[0], u[1] + v[1], u[2] + v[2]]);
    if s1 == 1 && s2 == 1 {
        0
    } else {
        3 - (s1 + s2 + dg)
    }
}

/// Só faces expostas, com AO nos cantos (mesma regra do engine JS).
pub fn mesh(g: &Grid) -> Mesh {
    let mut m = Mesh::default();
    for (p, rgb) in g.sorted() {
        let rgb = [(rgb >> 16) & 255, (rgb >> 8) & 255, rgb & 255].map(|c| c as f32 / 255.0);
        for (n, c) in FACES {
            if g.solid(p[0] + n[0], p[1] + n[1], p[2] + n[2]) {
                continue;
            }
            let s = (m.pos.len() / 3) as u32;
            let ao = c.map(|q| corner_ao(g, p, n, q));
            for i in 0..4 {
                let sh = AO_LEVEL[ao[i]];
                m.pos.extend((0..3).map(|a| (p[a] + c[i][a]) as f32));
                m.nor.extend(n.map(|v| v as f32));
                m.col.extend(rgb.map(|v| v * sh));
            }
            // Vira a diagonal do quad para a AO não "dobrar" errado.
            if ao[0] + ao[2] > ao[1] + ao[3] {
                m.idx.extend([s + 1, s + 2, s + 3, s + 1, s + 3, s]);
            } else {
                m.idx.extend([s, s + 1, s + 2, s, s + 2, s + 3]);
            }
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rot90_four_times_is_identity_and_keeps_footprint() {
        for (x, z) in [(0, 0), (-2, 1), (3, -4)] {
            let mut p = [x, z];
            for _ in 0..4 {
                p = rot90(p[0], p[1], 90);
            }
            assert_eq!(p, [x, z]);
        }
        // Caixa 4x2 centrada: x -2..1, z -1..0 → girada vira x -1..0, z -2..1.
        let mut g = Grid::default();
        g.fill([-2, 0, -1], [1, 0, 0], 1);
        let mut r = Grid::default();
        r.stamp(&g, [0; 3], 90);
        assert_eq!(r.bounds(), ([-1, 0, -2], [0, 0, 1]));
    }

    #[test]
    fn single_voxel_has_six_faces() {
        let mut g = Grid::default();
        g.put(0, 0, 0, 0xffffff);
        let m = mesh(&g);
        assert_eq!(m.idx.len(), 36);
        assert_eq!(m.pos.len(), 24 * 3);
    }

    #[test]
    fn hidden_faces_are_culled() {
        let mut g = Grid::default();
        g.fill([0, 0, 0], [1, 0, 0], 0xffffff);
        assert_eq!(mesh(&g).idx.len(), 10 * 6);
    }
}
