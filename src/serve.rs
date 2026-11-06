//! Página local: galeria de assets, aprovação e exportação. Só escuta em 127.0.0.1.
//! Segurança: /api/* exige o token aleatório da sessão (na URL aberta pelo programa)
//! e Host/Origin locais, para que outros sites abertos no navegador não consigam usar a API.
use crate::analyze;
use crate::export::export;
use crate::library::{self as lib, Asset};
use crate::voxel::{mesh, Grid};
use std::hash::{BuildHasher, Hasher};
use std::path::Path;
use tiny_http::{Header, Method, Request, Response, Server};

const INDEX: &str = include_str!("../web/index.html");
/// three.js r160 embutido (MIT, web/three.LICENSE): funciona sem internet.
const THREE: &str = include_str!("../web/three.module.min.js");

pub fn run(root: &Path, port: u16, open: bool) -> Result<(), String> {
    let server = Server::http(("127.0.0.1", port))
        .or_else(|_| Server::http(("127.0.0.1", 0)))
        .map_err(|e| e.to_string())?;
    let port = server
        .server_addr()
        .to_ip()
        .map(|a| a.port())
        .ok_or("endereço inválido")?;
    let token = new_token();
    let url = format!("http://127.0.0.1:{port}/?t={token}");
    println!("voxel-hub em {url}  (biblioteca: {})", root.display());
    println!("Feche esta janela para encerrar.");
    if open {
        open_browser(&url);
    }
    for mut req in server.incoming_requests() {
        let res = match check(&req, port, &token) {
            Ok(()) => route(root, &mut req),
            Err(e) => {
                let _ = req.respond(json(&serde_json::json!({ "error": e })).with_status_code(403));
                continue;
            }
        };
        let _ = match res {
            Ok(r) => req.respond(r),
            Err(e) => req.respond(json(&serde_json::json!({ "error": e })).with_status_code(400)),
        };
    }
    Ok(())
}

type Res = Response<std::io::Cursor<Vec<u8>>>;

fn route(root: &Path, req: &mut Request) -> Result<Res, String> {
    let url = req.url().split('?').next().unwrap_or("").to_string();
    let seg: Vec<&str> = url.split('/').filter(|s| !s.is_empty()).collect();
    if seg.iter().any(|s| !valid(s)) {
        return Err("caminho inválido".into());
    }
    match (req.method(), seg.as_slice()) {
        (Method::Get, []) => Ok(Response::from_string(INDEX)
            .with_header(header("Content-Type", "text/html; charset=utf-8"))),
        (Method::Get, ["three.module.js"]) => {
            Ok(Response::from_string(THREE).with_header(header("Content-Type", "text/javascript")))
        }
        (Method::Get, ["api", "assets"]) => Ok(json(&lib::list_assets(root))),
        (Method::Get, ["api", "scenes"]) => Ok(json(&lib::list_scenes(root))),
        (Method::Get, ["api", kind, id, "mesh"]) => Ok(mesh_json(&grid(root, kind, id)?)),
        (Method::Get, ["api", kind, id, "export", fmt]) => {
            let g = grid(root, kind, id)?;
            let scale = if *fmt == "stl" { 1.0 } else { 0.1 };
            let data = export(&g, fmt, scale)?;
            Ok(Response::from_data(data)
                .with_header(header("Content-Type", "application/octet-stream"))
                .with_header(header(
                    "Content-Disposition",
                    &format!("attachment; filename=\"{id}.{fmt}\""),
                )))
        }
        (Method::Post, ["api", "assets", id, "review"]) => {
            let mut body = String::new();
            req.as_reader()
                .read_to_string(&mut body)
                .map_err(|e| e.to_string())?;
            let v: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
            let action = v["action"].as_str().unwrap_or("");
            let comment = v["comment"].as_str().map(String::from);
            let a: Asset = lib::review(root, id, action, comment)?;
            Ok(json(&a))
        }
        (Method::Get, ["api", "config"]) => {
            let o = analyze::Opts::from_env();
            Ok(json(&serde_json::json!({
                "key_set": analyze::api_key().is_some(),
                "model": o.model, "effort": o.effort, "models": analyze::MODELS.map(|m| m.0),
            })))
        }
        (Method::Post, ["api", "cli"]) => {
            let mut body = String::new();
            req.as_reader()
                .read_to_string(&mut body)
                .map_err(|e| e.to_string())?;
            let v: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
            let args: Vec<String> = v["args"]
                .as_array()
                .ok_or("args deve ser uma lista")?
                .iter()
                .map(|a| a.as_str().unwrap_or("").to_string())
                .collect();
            Ok(json(&run_cli(root, &args)?))
        }
        (Method::Post, ["api", "analyze", scene, ext]) => {
            let mut img = Vec::new();
            req.as_reader()
                .read_to_end(&mut img)
                .map_err(|e| e.to_string())?;
            Ok(json(&analyze::analyze(
                root,
                &img,
                &format!("{scene}.{ext}"),
                scene,
                false,
                &analyze::Opts::from_env().with_model(query(req.url(), "model")),
            )?))
        }
        _ => Ok(Response::from_string("não encontrado").with_status_code(404)),
    }
}

fn query(url: &str, key: &str) -> Option<String> {
    let q = url.split_once('?')?.1;
    q.split('&')
        .filter_map(|kv| kv.split_once('='))
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v.to_string())
}

/// Comandos que o menu da página pode rodar (nunca "serve").
const CLI_ALLOWED: [&str; 4] = ["list", "review", "analyze", "export"];

/// Roda o próprio executável com --json, sem shell (argumentos vão direto, sem interpretação).
fn run_cli(root: &Path, args: &[String]) -> Result<serde_json::Value, String> {
    let cmd = args.first().ok_or("informe um comando")?;
    if !CLI_ALLOWED.contains(&cmd.as_str()) {
        return Err(format!(
            "comando não permitido: {cmd} (use {})",
            CLI_ALLOWED.join(", ")
        ));
    }
    if args
        .iter()
        .any(|a| a == "--root" || a.starts_with("--root="))
    {
        return Err("--root não é permitido pelo menu".into());
    }
    let root = std::fs::canonicalize(root).map_err(|e| e.to_string())?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    // Roda dentro da biblioteca: caminhos relativos (ex.: -o exports) caem nela.
    let out = std::process::Command::new(exe)
        .current_dir(&root)
        .arg("--root")
        .arg(&root)
        .arg("--json")
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let err = String::from_utf8_lossy(&out.stderr).into_owned();
    let output = serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text + &err));
    Ok(serde_json::json!({ "ok": out.status.success(), "output": output }))
}

/// /api/* só com token da sessão; qualquer rota só com Host/Origin locais (contra DNS rebinding).
fn check(req: &Request, port: u16, token: &str) -> Result<(), String> {
    let hdr = |name: &'static str| {
        req.headers()
            .iter()
            .find(|h| h.field.equiv(name))
            .map(|h| h.value.as_str().to_string())
    };
    let local = [format!("127.0.0.1:{port}"), format!("localhost:{port}")];
    if !hdr("Host").is_some_and(|h| local.contains(&h)) {
        return Err("host não permitido".into());
    }
    if let Some(o) = hdr("Origin") {
        if !local.iter().any(|l| o == format!("http://{l}")) {
            return Err("origem não permitida".into());
        }
    }
    if req.url().starts_with("/api/") {
        let q = req.url().split_once('?').map(|(_, q)| q).unwrap_or("");
        let in_query = q.split('&').any(|kv| kv == format!("t={token}"));
        if !(in_query || hdr("X-Voxel-Token").as_deref() == Some(token)) {
            return Err("token inválido; reabra pelo programa".into());
        }
    }
    Ok(())
}

/// 128 bits do gerador do sistema (RandomState é semeado pelo SO a cada instância).
fn new_token() -> String {
    (0..2)
        .map(|i| {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_u64(i);
            format!("{:016x}", h.finish())
        })
        .collect()
}

fn grid(root: &Path, kind: &str, id: &str) -> Result<Grid, String> {
    match kind {
        "assets" => lib::asset_grid(root, &lib::load_asset(root, id)?, None),
        "scenes" => lib::scene_grid(root, id),
        _ => Err("tipo inválido".into()),
    }
}

/// Ids e nomes: só letras, números, '-', '_' e '.' (sem "..").
fn valid(s: &str) -> bool {
    s != ".."
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
}

fn mesh_json(g: &Grid) -> Res {
    let m = mesh(g);
    json(
        &serde_json::json!({ "pos": m.pos, "nor": m.nor, "col": m.col, "idx": m.idx, "count": g.vox.len() }),
    )
}

fn json<T: serde::Serialize>(v: &T) -> Res {
    Response::from_string(serde_json::to_string(v).unwrap_or_default())
        .with_header(header("Content-Type", "application/json"))
}

fn header(k: &str, v: &str) -> Header {
    Header::from_bytes(k.as_bytes(), v.as_bytes()).expect("header válido")
}

fn open_browser(url: &str) {
    let r = if cfg!(windows) {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(url).spawn()
    };
    if r.is_err() {
        println!("Abra no navegador: {url}");
    }
}
