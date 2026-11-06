mod analyze;
mod export;
mod library;
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
        Cmd::Analyze {
            image,
            scene,
            force,
        } => {
            let bytes = std::fs::read(&image).map_err(|e| format!("{}: {e}", image.display()))?;
            let name = image
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if !as_json {
                println!("Analisando {name} com o Claude (pode levar alguns minutos)...");
            }
            let out = analyze::analyze(root, &bytes, &name, &scene, force)?;
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
            let mut files = vec![];
            for f in &format {
                let scale = if f == "stl" { voxel_mm } else { voxel_m };
                let path = out.join(format!("{name}.{f}"));
                std::fs::write(&path, export::export(&g, f, scale)?).map_err(|e| e.to_string())?;
                if !as_json {
                    println!("{f}: {} ({} voxels)", path.display(), g.vox.len());
                }
                files.push(path.display().to_string());
            }
            if as_json {
                println!("{}", json!({ "files": files, "voxels": g.vox.len() }));
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
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from))
        .filter(|d| d.join("assets").is_dir())
        .unwrap_or(cwd)
}
