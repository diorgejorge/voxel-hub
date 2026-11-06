# voxel-hub — instruções para o Claude

Ferramenta Rust `voxel` (binário único): imagem isométrica → assets voxel aprovados → cena → GLB/VOX/STL.
Respostas ao usuário em PT-BR, curtas. Detalhes e formatos dos arquivos: [README.md](README.md).

## Usar em outra conversa do Claude

- Claude Code com este repo aberto: este arquivo já é lido; rode os comandos abaixo.
- Fora do repo: deixe `voxel` (ou `voxel.exe`) no PATH e passe `--root <pasta da biblioteca>`.
- Chat comum do claude.ai não roda programas: use o Claude Code (app, web ou terminal) com este repo.

## Rodar a ferramenta a partir do chat

- Nuvem (sessão do Claude Code): `cargo build --release -q` e depois `./target/release/voxel --json <comando>`.
- PC do usuário (Windows, `C:\home\Repos\voxel-hub`): via Remote Control, `cargo run --release -- --json <comando>` ou `voxel.exe --json <comando>`.
- Sempre use `--json`: a saída é um objeto JSON; erro vem como `{"error": "..."}` e código de saída 1.

## Comandos

| Comando | O que faz |
|---|---|
| `list` | assets (com status) e cenas |
| `review <id> approve\|adjust\|discard [-m "comentário"]` | decisão de aprovação (máx 2 ajustes) |
| `analyze <imagem> --scene <nome> [--model id] [--effort low] [--force]` | imagem → API do Claude → assets novos `pending` + `scenes/<nome>/scene.json` |
| `export (--asset <id>\|--scene <nome>) [--format glb,vox,stl] [-o exports]` | gera arquivos |
| `dump oak\|crag` | voxels do gerador (comparar com o HTML) |
| `serve [--port 7878] [--no-open]` | página local (o que o usuário usa com dois cliques) |

Exemplos:

```
voxel --json list
voxel --json review crate-wood-01 adjust -m "mais alto"
voxel --json export --scene demo --format glb,stl -o exports
```

`analyze` precisa de `ANTHROPIC_API_KEY` (só variável de ambiente; nunca gravar em disco). Saída inclui `cost_usd`.

No chat, prefira analisar a imagem você mesmo e escrever o `scene.json` (sem custo de API).

## Regras que não mudam

- `assets/` e `scenes/` são a biblioteca local do usuário: estão no .gitignore e nunca são commitados. Exemplos ficam em `examples/`.
- Nunca fazer push sem testar localmente e sem o usuário aprovar a subida.
- Nunca rode `review ... approve`/`discard` sem o usuário ter dito isso na conversa; você só propõe.
- Só rode `analyze` (pago) quando o usuário pedir, uma vez por imagem, e informe o `cost_usd`. `--force` sobrescreve a cena: só com confirmação.
- Cena só monta com assets `approved`. Rotação em múltiplos de 90°. Chão y 0..3, objetos em y = 4.
- Novas peças: `src/pieces.rs` (adicionar em `NAMES`, no `match` e no enum `piece` de `src/analyze.rs`).
- Antes de commitar: `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test`.
