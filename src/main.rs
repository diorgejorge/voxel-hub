mod analyze;
mod export;
mod gen;
mod library;
mod organic;
mod pieces;
mod serve;
mod voxel;

use clap::{Parser, Subcommand};
use serde_json::json;
use std::path::PathBuf;

/// Biblioteca de assets voxel. Sem argumentos (dois cliques) abre a página no navegador.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Pasta da biblioteca (com assets/ e scenes/). Padrão: pasta atual ou a do executável.
    #[arg(long, global = true)]
    root: Option<PathBuf>,
    /// Saída em JSON (para scripts e para o Claude).
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Abre a página local de aprovação e exportação.
    Serve {
        #[arg(long, default_value_t = 7878)]
        port: u16,
        /// Não abre o navegador.
        #[arg(long)]
        no_open: bool,
    },
    /// Lista assets e cenas.
    List,
    /// Aprova, ajusta ou descarta um asset.
    Review {
        id: String,
        #[arg(value_parser = ["approve", "adjust", "discard"])]
        action: String,
        /// Comentário (obrigatório na prática para "adjust").
        #[arg(short, long)]
        message: Option<String>,
    },
    /// Analisa uma imagem com a API do Claude: cria assets novos (pendentes) e o rascunho de scenes/<cena>/scene.json.
    Analyze {
        image: PathBuf,
        /// Nome da cena a criar.
        #[arg(long)]
        scene: String,
        /// Sobrescreve a cena se já existir.
        #[arg(long)]
        force: bool,
        /// Modelo: claude-haiku-5-5, claude-sonnet-5-5 ou claude-opus-5-5 (padrão: VOXEL_MODEL ou sonnet).
        #[arg(long)]
        model: Option<String>,
        /// Esforço: low, medium, high (padrão: VOXEL_EFFORT ou low).
        #[arg(long)]
        effort: Option<String>,
    },
    /// Voxels brutos de um gerador portado (oak, crag) em coordenadas do HTML, como JSON.
    Dump {
        #[arg(value_parser = ["oak", "crag"])]
        piece: String,
    },
    /// Exporta um asset ou uma cena.
    Export {
        #[arg(long, conflicts_with = "scene", required_unless_present = "scene")]
        asset: Option<String>,
        #[arg(long)]
        scene: Option<String>,
        /// Formatos separados por vírgula: glb, vox, stl.
        #[arg(long, default_value = "glb,vox,stl", value_delimiter = ',')]
        format: Vec<String>,
        /// Pasta de saída.
        #[arg(short, long, default_value = "exports")]
        out: PathBuf,
        /// Tamanho do voxel no GLB, em metros.
        #[arg(long, default_value_t = 0.1)]
        voxel_m: f32,
        /// Tamanho do voxel no STL, em milímetros.
        #[arg(long, default_value_t = 1.0)]
        voxel_mm: f32,
    },
}

fn main() {
    let cli = Cli::parse();
    let root = cli.root.unwrap_or_else(default_root);
    let as_json = cli.json;
    if let Err(e) = run(&root, cli.cmd, as_json) {
        if as_json {
            println!("{}", json!({ "error": e }));
        } else {
            eprintln!("erro: {e}");
        }
        std::process::exit(1);
    }
}

fn run(root: &std::path::Path, cmd: Option<Cmd>, as_json: bool) -> Result<(), String> {
    match cmd.unwrap_or(Cmd::Serve {
        port: 7878,
        no_open: false,
    }) {
        Cmd::Serve { port, no_open } => serve::run(root, port, !no_open),
        Cmd::List if as_json => {
            let out = json!({ "assets": library::list_assets(root), "scenes": library::list_scenes(root) });
            println!("{out}");
            Ok(())
        }
        Cmd::List => {
            for a in library::list_assets(root) {
                println!(
                    "{:<24} {:<6} {:<10} {}",
                    a.id,
                    a.kind,
                    a.status,
                    a.tags.join(", ")
                );
            }
            for s in library::list_scenes(root) {
                println!("cena: {s}");
            }
            Ok(())
        }
        Cmd::Review {
            id,
            action,
            message,
        } => {
            let a = library::review(root, &id, &action, message)?;
            if as_json {
                println!("{}", serde_json::to_string(&a).unwrap_or_default());
                return Ok(());
            }
            println!("{}: {} (rodadas de ajuste: {})", a.id, a.status, a.rounds);
            Ok(())
        }
        Cmd::Dump { piece } => {
            let g = if piece == "oak" {
                organic::oak(true)
            } else {
                organic::crag(true)
            };
            let v: Vec<[i64; 4]> = g
                .sorted()
                .into_iter()
                .map(|(p, c)| [p[0] as i64, p[1] as i64, p[2] as i64, c as i64])
                .collect();
            println!("{}", serde_json::to_string(&v).unwrap_or_default());
            Ok(())
        }
        Cmd::Analyze {
            image,
            scene,
            force,
            model,
            effort,
        } => {
            let mut opts = analyze::Opts::from_env().with_model(model);
            if let Some(e) = effort {
                opts.effort = e;
            }
            let bytes = std::fs::read(&image).map_err(|e| format!("{}: {e}", image.display()))?;
            let name = image
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if !as_json {
                println!("Analisando {name} com o Claude (pode levar alguns minutos)...");
            }
            let out = analyze::analyze(root, &bytes, &name, &scene, force, &opts, &|| false)?;
            if as_json {
                println!("{out}");
            } else {
                println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
                println!(
                    "Aprove os assets novos na página (dois cliques no voxel) para montar a cena."
                );
            }
            Ok(())
        }
        Cmd::Export {
            asset,
            scene,
            format,
            out,
            voxel_m,
            voxel_mm,
        } => {
            let (name, g) = match (asset, scene) {
                (Some(id), _) => {
                    let a = library::load_asset(root, &id)?;
                    let g = library::asset_grid(root, &a, None)?;
                    (id, g)
                }
                (_, Some(s)) => {
                    let g = library::scene_grid(root, &s)?;
                    (s, g)
                }
                _ => unreachable!("clap exige --asset ou --scene"),
            };
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            if format.iter().any(|f| f == "stl") && voxel_mm < 0.8 {
                eprintln!(
                    "aviso: voxel de {voxel_mm} mm é menor que 0,8 mm; pode não imprimir bem"
                );
            }
            // Um formato com erro não impede os outros.
            let (mut files, mut errors) = (vec![], vec![]);
            for f in &format {
                let scale = if f == "stl" { voxel_mm } else { voxel_m };
                let path = out.join(format!("{name}.{f}"));
                match export::export(&g, f, scale)
                    .and_then(|b| std::fs::write(&path, b).map_err(|e| e.to_string()))
                {
                    Ok(()) => {
                        if !as_json {
                            println!("{f}: {} ({} voxels)", path.display(), g.vox.len());
                        }
                        files.push(path.display().to_string());
                    }
                    Err(e) => {
                        if !as_json {
                            eprintln!("{f}: {e}");
                        }
                        errors.push(format!("{f}: {e}"));
                    }
                }
            }
            if files.is_empty() {
                return Err(errors.join("; "));
            }
            if as_json {
                println!(
                    "{}",
                    json!({ "files": files, "voxels": g.vox.len(), "errors": errors })
                );
            }
            Ok(())
        }
    }
}

/// Pasta atual se tiver assets/; senão a pasta do executável (caso dos dois cliques).
fn default_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_default();
    if cwd.join("assets").is_dir() {
        return cwd;
    }
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from));
    match exe_dir {
        // Primeiro uso: a biblioteca nasce ao lado do executável (exceto em builds do cargo).
        Some(d)
            if d.join("assets").is_dir() || !d.components().any(|c| c.as_os_str() == "target") =>
        {
            d
        }
        _ => cwd,
    }
}
