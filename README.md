# voxel-hub

Ferramenta para montar cenas voxel a partir de uma biblioteca de assets aprovados.
Um único executável para Windows, macOS e Linux.

## Como funciona

1. **Imagem → cena**: a imagem isométrica é decomposta em objetos. Cada objeto reaproveita um asset da biblioteca ou vira um asset novo.
2. **Aprovação**: asset novo entra como `pending`. Você aprova, sugere mudança com comentário (até 2 vezes) ou exclui. Nada entra aprovado sem você.
3. **Cena**: `scene.json` lista o chão e as instâncias (asset, posição, rotação). Ela só monta quando todos os assets estão `approved`.
4. **Exportação**: GLB (Godot/Blender), VOX (MagicaVoxel) e STL (impressão 3D).

A biblioteca (`assets/`, `scenes/`, `exports/`) é local e de cada usuário: fica ao lado do executável, é criada no primeiro uso e **não vai para o GitHub** (está no `.gitignore`). Exemplos do repositório ficam em `examples/`.

## Uso pela interface (sem terminal)

1. Baixe o executável na página **Releases** do GitHub (`voxel-windows.exe`, `voxel-macos` ou `voxel-linux`). Não precisa instalar Rust.
2. Coloque-o numa pasta sua e dê dois cliques. O navegador abre a página local.
   - No Windows, um executável baixado e não assinado pode mostrar "O Windows protegeu o computador": clique em **Mais informações → Executar assim mesmo**.
3. Na página: **Biblioteca** (assets e cenas em 3D), **Aprovar** (✅ aprovar, ✏️ ajustar com comentário até 2 vezes, ❌ descartar), **Nova cena** (criar assets a partir de uma imagem) e **Exportar** (GLB, VOX, STL).
   - Só uma análise de imagem roda por vez. Se você cancelar, a análise já paga termina e o resultado é descartado; nenhuma cena é sobrescrita.
4. **Criar assets a partir de imagem** usa a API do Claude e tem custo por imagem (mostrado após cada análise). Defina a chave uma vez e reabra o programa:
   - Windows: `setx ANTHROPIC_API_KEY "sk-ant-..."`
   - macOS/Linux: `export ANTHROPIC_API_KEY=sk-ant-...` no seu perfil do shell.

   A chave só é lida da variável de ambiente; o programa nunca a grava em disco. Modelo padrão: `claude-sonnet-5-5` com esforço `low`; troque na página ou com `VOXEL_MODEL` / `VOXEL_EFFORT`.
5. Feche a janela do programa para encerrar.

Para testar: copie `examples/assets` e `examples/scenes` para a pasta do executável.

Segurança: a página só aceita chamadas com o token aleatório da URL aberta pelo programa e só de `127.0.0.1`/`localhost`.

## CLI (integração com outras ferramentas)

O CLI é para scripts e integrações (por exemplo, o Claude no chat). Use sempre `--json`: a saída é um objeto JSON; erro vem como `{"error": "..."}` com código de saída 1. `--root <pasta>` escolhe a biblioteca (padrão: a pasta atual se tiver `assets/`, senão a do executável).

| Comando | O que faz |
|---|---|
| `list` | assets (com status) e cenas |
| `review <id> approve\|adjust\|discard [-m "comentário"]` | decisão de aprovação (máx 2 ajustes) |
| `analyze <imagem> --scene <nome> [--model id] [--effort low] [--force]` | imagem → API do Claude → assets `pending` + `scenes/<nome>/scene.json` |
| `export (--asset <id>\|--scene <nome>) [--format glb,vox,stl] [-o exports] [--voxel-m 0.1] [--voxel-mm 1]` | gera os arquivos |
| `dump oak\|crag` | voxels do gerador (para comparar com o HTML) |
| `serve [--port 7878] [--no-open]` | abre a página local |

Exemplos:

```
$ voxel --json list
{"assets":[{"id":"barrel-wood-01","status":"approved","piece":"barrel",...}],"scenes":["demo"]}

$ voxel --json review crate-wood-01 adjust -m "mais alto"
$ voxel --json review crate-wood-01 approve

$ voxel --json analyze cabana.png --scene cabana --model claude-haiku-5-5
{"scene":"cabana","created_pending":["cabin-01"],"instances":5,"dropped":[],"manual":[],"cost_usd":0.004,...}

$ voxel --json export --scene demo --format glb,vox,stl -o exports
{"files":["exports/demo.glb","exports/demo.vox","exports/demo.stl"],"voxels":34675,"errors":[]}

$ voxel --json export --asset nao-existe
{"error":"assets/nao-existe/asset.json: No such file or directory (os error 2)"}
```

Cada formato é exportado de forma independente: se um falhar, os outros saem e a falha aparece em `errors`.

Compilar: `cargo build --release` (Rust estável). Antes de commitar: `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test`.

## Formatos dos arquivos

`assets/<id>/asset.json` (o nome da pasta tem de ser igual ao `id`):

```json
{
  "id": "barrel-wood-01",
  "type": "param",
  "piece": "barrel",
  "params": { "r": 4, "h": 8 },
  "palette": { "barrel": "#7a5238", "barrelD": "#5f3f2c", "hoop": "#5b5f68" },
  "tags": ["madeira", "prop"],
  "status": "pending",
  "rounds": 0
}
```

- `type`: `"param"` (peça + params + paleta) ou `"fixed"` (voxels prontos em `voxels.json`).
- `status`: `pending` → `approved` ou `discarded` ("Excluir" na página = `discarded`, some da biblioteca mas o arquivo fica). `approved` guarda a data da aprovação e é apagado ao sugerir mudança ou excluir. `rounds` conta os ajustes (máx 2); `comment` guarda o último pedido.
- Pilha de rochas: `{"piece": "rockpile", "params": {"size": 32, "rocks": [{"x": 15, "z": 14, "r": 12, "h": 19, "e": 0.95}, {"x": 23, "z": 23, "r": 5, "h": 6, "dome": 1}]}}`.

`scenes/<nome>/scene.json`:

```json
{
  "ground": { "slab": [0, 63, 0, 63], "color": "#6f9a4b" },
  "instances": [
    { "asset": "barrel-wood-01", "at": [10, 10], "rot": 0 },
    { "asset": "barrel-wood-01", "at": [20, 14], "rot": 90, "tint": { "barrel": "#6a4630" } },
    { "asset": "crate-wood-01", "at": [20, 14], "y": 8, "rot": 0 }
  ]
}
```

- `slab`: `[x0, x1, z0, z1]` do chão, em voxels de design. `at`: `[x, z]` do centro da base. `y`: altura extra para empilhar (padrão 0).
- `rot`: 0, 90, 180 ou 270. `tint`: troca cores da paleta só nesta instância.
- A peça inteira tem de caber na grade 128×128; senão a cena dá erro dizendo qual instância sair.

## Estrutura

```
assets/<id>/asset.json     metadados; type "param" (peça + params + paleta) ou "fixed" (+ voxels.json)
scenes/<nome>/scene.json   chão + instâncias de assets aprovados
src/                       núcleo (voxel.rs), peças (pieces.rs), biblioteca, exportação, servidor local
web/index.html             interface local (three.js embutido, funciona offline)
tools/compare-html.mjs     compara os geradores com os HTMLs originais
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
- **VOX** (MagicaVoxel): até 255 cores. Cena maior que 256 em algum eixo vira vários modelos posicionados no mesmo arquivo.
- **STL** (impressão): milímetros, Z para cima. Aviso se o voxel for menor que 0,8 mm.
