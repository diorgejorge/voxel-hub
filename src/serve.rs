//! Página local: galeria de assets, aprovação e exportação. Só escuta em 127.0.0.1.
use crate::export::export;
use crate::library::{self as lib, Asset};
use crate::voxel::{mesh, Grid};
use std::path::Path;
use tiny_http::{Header, Method, Request, Response, Server};

const INDEX: &str = include_str!("../web/index.html");
/// three.js r160 embutido (MIT, web/three.LICENSE): funciona sem internet.
const THREE: &str = include_str!("../web/three.module.min.js");

pub fn run(root: &Path, port: u16, open: bool) -> Result<(), String> {
    let server = Server::http(("127.0.0.1", port))
        .or_else(|_| Server::http(("127.0.0.1", 0)))
        .map_err(|e| e.to_string())?;
    let url = format!("http://{}", server.server_addr());
    println!("voxel-hub em {url}  (biblioteca: {})", root.display());
    println!("Feche esta janela para encerrar.");
    if open {
        open_browser(&url);
    }
    for mut req in server.incoming_requests() {
        let res = route(root, &mut req);
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
        _ => Ok(Response::from_string("não encontrado").with_status_code(404)),
    }
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
