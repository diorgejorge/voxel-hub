# voxel-hub

Ferramenta para montar cenas voxel a partir de uma biblioteca de assets aprovados.
Um único executável para Windows, macOS e Linux.

## Uso sem terminal

1. Baixe o executável na página **Releases** do GitHub (`voxel-windows.exe`, `voxel-macos` ou `voxel-linux`). Não precisa instalar Rust.
2. Coloque-o numa pasta sua. A biblioteca (`assets/`, `scenes/`) é criada ao lado dele e é só sua: não vai para o GitHub.
3. Dê dois cliques. O navegador abre a página com Biblioteca, Aprovar (✅ aprovar, ✏️ ajustar com comentário até 2 vezes, ❌ descartar), Nova cena, Exportar (GLB, VOX, STL) e Avançado (roda comandos do CLI).
4. **Nova cena a partir de imagem** usa a API do Claude e tem custo por imagem (mostrado após cada análise). Defina a chave uma vez e reabra o programa:
   - Windows: `setx ANTHROPIC_API_KEY "sk-ant-..."`
   - macOS/Linux: `export ANTHROPIC_API_KEY=sk-ant-...` no seu perfil do shell.
   Modelo padrão: `claude-sonnet-5-5` com esforço `low`; troque na página ou com `VOXEL_MODEL` / `VOXEL_EFFORT`.
   Os objetos novos entram como *pending*; a cena só monta quando todos forem aprovados.
5. Feche a janela do programa para encerrar.

Exemplos para testar: copie `examples/assets` e `examples/scenes` para a pasta do executável.

Segurança: a página só aceita chamadas com o token aleatório da URL aberta pelo programa e só de `127.0.0.1`/`localhost`.

## Uso no terminal

```
voxel list
voxel review crate-wood-01 approve
voxel review crate-wood-01 adjust -m "mais alto"
voxel analyze cabana.png --scene cabana --model claude-haiku-5-5   # usa ANTHROPIC_API_KEY
voxel --json list                            # saída JSON (scripts e Claude)
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

- Padrão dos HTMLs para tudo: cada voxel de design vira 2×2×2 voxels finos (S = 2), cor por hash/rampa e AO por vértice em cor linear. Cena 128×128 de design = 256 finos (máximo do .vox): fidelidade à imagem antes de tamanho.
- Y para cima. Chão ocupa y 0..3 (finos); objetos apoiam em y = 4. Instância aceita `y` (design) para empilhar.
- Origem do asset: centro da base. Rotação só em múltiplos de 90°.
- Cena só aceita assets com `status: "approved"`.
- Peças (medidas em voxels de design): `box` (w, h, d), `cyl` (r, h), `ball` (r), `barrel` (r, h),
  `oak` (Carvalho Antigo), `crag` (Montanha de Pedra), `rockpile` (pilha de rochas:
  `rocks: [{x, z, r, h, e, dome, lit}]`, `size`). `oak`/`crag`/`rockpile` têm placa de chão; `tile: 0` tira.
- `crag` é uma `rockpile` de 7 rochas. Conferir com o HTML original: `node tools/compare-html.mjs <html> oak|crag`.
- `voxels.json` (asset fixo): `{"size": [x, y, z], "palette": ["#hex", ...], "rle": [[qtd, cor], ...]}`,
  ordem x → z → y, cor 0 = vazio, n = palette[n-1].

### Exportação

- **GLB** (Godot/Blender): Y para cima, cor por vértice (COLOR_0 linear) com AO. No Godot, se ficar branco: Vertex Color → Use As Albedo.
- **VOX** (MagicaVoxel): até 256 por eixo e 255 cores.
- **STL** (impressão): milímetros, Z para cima. Aviso se o voxel for menor que 0,8 mm.
