# voxel-hub — instruções para o Claude

Ferramenta Rust `voxel` (binário único). Respostas ao usuário em PT-BR, curtas.

## Rodar a ferramenta a partir do chat

- Nuvem (sessão do Claude Code): `cargo build --release -q` e depois `./target/release/voxel --json <comando>`.
- PC do usuário (Windows, `C:\home\Repos\voxel-hub`): via Remote Control, `cargo run --release -- --json <comando>` ou `voxel.exe --json <comando>`.
- Sempre use `--json`: a saída é um objeto JSON; erro vem como `{"error": "..."}` e código de saída 1.

## Comandos

| Comando | O que faz |
|---|---|
| `list` | assets (com status) e cenas |
| `review <id> approve\|adjust\|discard [-m "comentário"]` | decisão de aprovação (máx 2 ajustes) |
| `analyze <imagem> --scene <nome> [--force]` | imagem → API do Claude → assets novos `pending` + `scenes/<nome>/scene.json` |
| `export (--asset <id>\|--scene <nome>) [--format glb,vox,stl] [-o exports]` | gera arquivos |
| `serve [--port 7878] [--no-open]` | página local (o que o usuário usa com dois cliques) |

`analyze` precisa de `ANTHROPIC_API_KEY` ou da chave salva pela página em `.voxel/key` (fora do git).

## Regras que não mudam

- Nada entra aprovado sem o usuário: só `review ... approve` quando ele pedir.
- Cena só monta com assets `approved`. Rotação em múltiplos de 90°. Chão y 0..3, objetos em y = 4.
- Novas peças: `src/pieces.rs` (adicionar em `NAMES`, no `match` e no enum `piece` de `src/analyze.rs`).
- Antes de commitar: `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test`.
