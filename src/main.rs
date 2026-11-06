mod export;
mod library;
mod pieces;
mod serve;
mod voxel;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Biblioteca de assets voxel. Sem argumentos (dois cliques) abre a página no navegador.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Pasta da biblioteca (com assets/ e scenes/). Padrão: pasta atual ou a do executável.
    #[arg(long, global = true)]
    root: Option<PathBuf>,
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
    if let Err(e) = run(&root, cli.cmd) {
        eprintln!("erro: {e}");
        std::process::exit(1);
    }
}

fn run(root: &std::path::Path, cmd: Option<Cmd>) -> Result<(), String> {
    match cmd.unwrap_or(Cmd::Serve {
        port: 7878,
        no_open: false,
    }) {
        Cmd::Serve { port, no_open } => serve::run(root, port, !no_open),
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
            println!("{}: {} (rodadas de ajuste: {})", a.id, a.status, a.rounds);
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
            for f in &format {
                let scale = if f == "stl" { voxel_mm } else { voxel_m };
                let path = out.join(format!("{name}.{f}"));
                std::fs::write(&path, export::export(&g, f, scale)?).map_err(|e| e.to_string())?;
                println!("{f}: {} ({} voxels)", path.display(), g.vox.len());
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
