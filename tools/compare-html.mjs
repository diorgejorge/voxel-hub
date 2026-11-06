// Compara um gerador HTML original com o porte em Rust, voxel a voxel.
// Uso: node tools/compare-html.mjs <arquivo.html> <oak|crag> [caminho do voxel]
// Imprime a impressão digital (contagem + FNV-1a) usada nos testes de src/organic.rs.
import { readFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';

const [html, piece, exe = 'target/debug/voxel'] = process.argv.slice(2);
const src = readFileSync(html, 'utf8');
let gen = src.slice(src.indexOf('const S = 2'), src.indexOf('// ===== MESHING WITH AO'));
// Fora do porte por decisão do projeto: samambaia, arbusto e juncos.
gen = gen.replace(/^\s*(fern|bush|reeds)\(.*\);\s*$/gm, '');
const run = new Function(`${gen}; buildScene(); return voxels;`);
const voxels = run();
const js = [];
for (const [k, c] of voxels) js.push([(k % 128) - 4, (Math.floor(k / 128) % 128) - 8, Math.floor(k / 16384) - 4, c]);
js.sort((a, b) => a[1] - b[1] || a[2] - b[2] || a[0] - b[0]);

let h = 0xcbf29ce484222325n;
const buf = Buffer.alloc(4);
for (const v of js) for (const n of v) { buf.writeInt32LE(n | 0); for (const b of buf) { h ^= BigInt(b); h = (h * 0x100000001b3n) & 0xffffffffffffffffn; } }
console.log(`js: ${js.length} voxels, fnv ${h}`);

const rs = JSON.parse(execFileSync(exe, ['dump', piece], { maxBuffer: 1 << 28 }).toString());
const key = (v) => `${v[0]},${v[1]},${v[2]}`;
const a = new Map(js.map((v) => [key(v), v[3]]));
const b = new Map(rs.map((v) => [key(v), v[3]]));
let missing = 0, extra = 0, color = 0;
for (const [k, c] of a) { if (!b.has(k)) missing++; else if (b.get(k) !== c) color++; }
for (const k of b.keys()) if (!a.has(k)) extra++;
console.log(`rust: ${rs.length} voxels | faltando ${missing}, sobrando ${extra}, cor diferente ${color}`);
process.exit(missing + extra + color ? 1 : 0);
