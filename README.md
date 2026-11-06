# voxel-hub

Ferramenta para montar cenas voxel a partir de uma biblioteca de assets aprovados.
Um único executável para Windows, macOS e Linux.

## Uso sem terminal

1. Baixe o executável da sua plataforma na aba **Actions** do GitHub (último build verde → Artifacts).
2. Coloque o `voxel.exe` (ou `voxel`) na pasta do repositório, ao lado de `assets/` e `scenes/`.
3. Dê dois cliques. O navegador abre com a biblioteca: escolha um asset para vê-lo girando e use
   ✅ Aprovar, ✏️ Ajustar (com comentário, no máximo 2 rodadas) ou ❌ Descartar. Os botões GLB, VOX e STL baixam o arquivo.
4. Feche a janela do programa para encerrar.

## Uso no terminal

```
voxel list
voxel review crate-wood-01 approve
voxel review crate-wood-01 adjust -m "mais alto"
voxel export --scene demo --format glb,vox,stl -o exports
voxel export --asset barrel-wood-01 --format stl --voxel-mm 2
voxel serve --port 7878 --no-open
```

Compilar: `cargo build --release` (Rust estável).

## Estrutura

```
assets/<id>/asset.json     metadados; type "param" (peça + params + paleta) ou "fixed" (+ voxels.json)
scenes/<nome>/scene.json   chão + instâncias de assets aprovados
src/                       núcleo (voxel.rs), peças (pieces.rs), biblioteca, exportação, servidor local
web/index.html             página de aprovação (three.js embutido, funciona offline)
```

### Regras

- Y para cima. Chão ocupa y 0..3; objetos apoiam em y = 4.
- Origem do asset: centro da base. Rotação só em múltiplos de 90°.
- Cena só aceita assets com `status: "approved"`.
- Peças disponíveis: `box` (w, h, d), `cyl` (r, h), `ball` (r), `barrel` (r, h).
- `voxels.json` (asset fixo): `{"size": [x, y, z], "palette": ["#hex", ...], "rle": [[qtd, cor], ...]}`,
  ordem x → z → y, cor 0 = vazio, n = palette[n-1].

### Exportação

- **GLB** (Godot/Blender): Y para cima, cor por vértice (COLOR_0 linear) com AO. No Godot, se ficar branco: Vertex Color → Use As Albedo.
- **VOX** (MagicaVoxel): até 256 por eixo e 255 cores.
- **STL** (impressão): milímetros, Z para cima. Aviso se o voxel for menor que 0,8 mm.
