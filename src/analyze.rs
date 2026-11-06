//! Imagem → API do Claude → decomposição em assets + rascunho de scene.json.
//! Nada é aprovado aqui: assets novos entram como "pending" e passam pela página de aprovação.
use crate::library::{self as lib, Asset};
use crate::pieces;
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::fs;
use std::path::Path;

/// (modelo, US$ por milhão de tokens de entrada, de saída). Ordem: do mais barato ao mais caro.
pub const MODELS: [(&str, f64, f64); 3] = [
    ("claude-haiku-5-5", 0.10, 0.50),
    ("claude-sonnet-5-5", 2.0, 10.0),
    ("claude-opus-5-5", 4.0, 20.0),
];
const DEFAULT_MODEL: &str = "claude-sonnet-5-5";

/// Modelo e esforço. Padrões trocáveis por VOXEL_MODEL e VOXEL_EFFORT.
pub struct Opts {
    pub model: String,
    pub effort: String,
}

impl Opts {
    pub fn from_env() -> Self {
        let env = |k: &str, d: &str| {
            std::env::var(k)
                .ok()
                .filter(|v| !v.is_empty())
                .unwrap_or(d.into())
        };
        Opts {
            model: env("VOXEL_MODEL", DEFAULT_MODEL),
            effort: env("VOXEL_EFFORT", "low"),
        }
    }

    pub fn with_model(mut self, m: Option<String>) -> Self {
        if let Some(m) = m.filter(|m| !m.is_empty()) {
            self.model = m;
        }
        self
    }
}
const API: &str = "https://api.anthropic.com/v1/messages";

#[derive(Deserialize)]
struct Plan {
    objects: Vec<Obj>,
    ground: GroundOut,
    instances: Vec<Inst>,
    notes: String,
}

#[derive(Deserialize)]
struct Obj {
    name: String,
    #[serde(rename = "match")]
    kind: String,
    asset_id: String,
    piece: String,
    params: Vec<Kv<f64>>,
    palette: Vec<Kv<String>>,
    tags: Vec<String>,
    /// Só para rockpile: as rochas da pilha.
    #[serde(default)]
    rocks: Vec<Map<String, Value>>,
}

#[derive(Deserialize)]
struct Kv<T> {
    key: String,
    value: T,
}

#[derive(Deserialize)]
struct GroundOut {
    x0: i32,
    x1: i32,
    z0: i32,
    z1: i32,
    color: String,
}

#[derive(Deserialize)]
struct Inst {
    asset_id: String,
    x: i32,
    z: i32,
    rot: i32,
    tint: Vec<Kv<String>>,
}

/// Chave só pela variável de ambiente (nunca gravada em disco pela ferramenta).
pub fn api_key() -> Option<String> {
    std::env::var("ANTHROPIC_API_KEY")
        .ok()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
}

pub fn media_type(file: &str) -> Result<&'static str, String> {
    match file
        .rsplit('.')
        .next()
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => Ok("image/png"),
        Some("jpg" | "jpeg") => Ok("image/jpeg"),
        Some("webp") => Ok("image/webp"),
        Some("gif") => Ok("image/gif"),
        _ => Err("imagem deve ser png, jpg, webp ou gif".into()),
    }
}

/// Analisa a imagem e grava: assets novos (pending), scenes/<scene>/scene.json e a imagem de referência.
pub fn analyze(
    root: &Path,
    image: &[u8],
    file_name: &str,
    scene: &str,
    force: bool,
    opts: &Opts,
) -> Result<Value, String> {
    let media = media_type(file_name)?;
    let &(model, price_in, price_out) =
        MODELS.iter().find(|m| m.0 == opts.model).ok_or_else(|| {
            format!(
                "modelo desconhecido: {} (use {})",
                opts.model,
                MODELS.map(|m| m.0).join(", ")
            )
        })?;
    let dir = root.join("scenes").join(scene);
    if dir.join("scene.json").exists() && !force {
        return Err(format!(
            "cena '{scene}' já existe (use outro nome ou --force)"
        ));
    }
    let key = api_key()
        .ok_or("sem chave da API: defina a variável ANTHROPIC_API_KEY e reabra o programa")?;
    let library: Vec<Value> = lib::list_assets(root)
        .iter()
        .filter(|a| a.status != "discarded")
        .map(|a| json!({"id": a.id, "type": a.kind, "piece": a.piece, "params": a.params, "palette": a.palette, "tags": a.tags, "status": a.status}))
        .collect();

    let mut body = json!({
        "model": model,
        "max_tokens": 16000,
        "output_config": {"effort": opts.effort, "format": {"type": "json_schema", "schema": schema()}},
        "system": system_prompt(&library),
        "messages": [{"role": "user", "content": [
            {"type": "image", "source": {"type": "base64", "media_type": media,
                "data": base64::engine::general_purpose::STANDARD.encode(image)}},
            {"type": "text", "text": "Decomponha esta imagem isométrica e monte a cena seguindo as regras."}
        ]}]
    });
    let agent = ureq::AgentBuilder::new().try_proxy_from_env(true).build();
    let mut req = agent
        .post(API)
        .timeout(std::time::Duration::from_secs(600))
        .set("x-api-key", &key)
        .set("anthropic-version", "2026-10-21");
    // Haiku não tem fallback no servidor; Sonnet/Opus caem para outro modelo se houver recusa.
    if !model.contains("haiku") {
        body["fallbacks"] = json!("default");
        req = req.set("anthropic-beta", "server-side-fallback-2026-07-01");
    }
    let res = req.send_json(body);
    let res: Value = match res {
        Ok(r) => r.into_json().map_err(|e| e.to_string())?,
        Err(ureq::Error::Status(code, r)) => {
            let msg = r.into_string().unwrap_or_default();
            return Err(format!("API do Claude respondeu {code}: {msg}"));
        }
        Err(e) => return Err(format!("falha de rede: {e}")),
    };
    match res["stop_reason"].as_str() {
        Some("refusal") => return Err("o modelo recusou a imagem".into()),
        Some("max_tokens") => {
            return Err("resposta cortada (max_tokens); tente uma imagem mais simples".into())
        }
        _ => {}
    }
    let text = res["content"]
        .as_array()
        .and_then(|c| c.iter().find(|b| b["type"] == "text"))
        .and_then(|b| b["text"].as_str())
        .ok_or("resposta sem texto")?;
    let plan: Plan =
        serde_json::from_str(text).map_err(|e| format!("JSON inválido do modelo: {e}"))?;
    let u = &res["usage"];
    let tokens = |k: &str| u[k].as_f64().unwrap_or(0.0);
    let cost = (tokens("input_tokens") * price_in + tokens("output_tokens") * price_out) / 1e6;
    let mut out = apply(root, plan, image, file_name, scene, &dir)?;
    out["model"] = json!(res["model"].as_str().unwrap_or(model));
    out["usage"] = u.clone();
    out["cost_usd"] = json!((cost * 10000.0).round() / 10000.0);
    Ok(out)
}

fn apply(
    root: &Path,
    plan: Plan,
    image: &[u8],
    file_name: &str,
    scene: &str,
    dir: &Path,
) -> Result<Value, String> {
    let existing: Vec<String> = lib::list_assets(root).into_iter().map(|a| a.id).collect();
    let (mut created, mut manual) = (vec![], vec![]);
    for o in plan.objects.iter().filter(|o| o.kind == "new") {
        if existing.contains(&o.asset_id) || created.contains(&o.asset_id) {
            continue;
        }
        if !pieces::NAMES.contains(&o.piece.as_str()) || !valid_id(&o.asset_id) {
            manual.push(json!({"name": o.name, "asset_id": o.asset_id, "reason": "nenhuma peça serve; criar à mão"}));
            continue;
        }
        let a = Asset {
            id: o.asset_id.clone(),
            kind: "param".into(),
            piece: Some(o.piece.clone()),
            params: o
                .params
                .iter()
                .map(|p| (p.key.clone(), json!(p.value)))
                .chain((!o.rocks.is_empty()).then(|| ("rocks".to_string(), json!(o.rocks))))
                .collect(),
            palette: o
                .palette
                .iter()
                .map(|p| (p.key.clone(), p.value.clone()))
                .collect(),
            tags: o.tags.clone(),
            status: "pending".into(),
            rounds: 0,
            comment: None,
            approved: None,
            extra: Map::from_iter([("source".to_string(), json!(file_name))]),
        };
        fs::create_dir_all(lib::asset_dir(root, &a.id)).map_err(|e| e.to_string())?;
        lib::save_asset(root, &a)?;
        created.push(a.id);
    }
    let known = |id: &String| existing.contains(id) || created.contains(id);
    let mut dropped = vec![];
    let instances: Vec<Value> = plan
        .instances
        .iter()
        .filter(|i| {
            let why = if !known(&i.asset_id) {
                "asset não existe"
            } else if !(0..lib::GRID).contains(&i.x) || !(0..lib::GRID).contains(&i.z) {
                "fora da grade da cena"
            } else {
                return true;
            };
            dropped.push(json!({"asset_id": i.asset_id, "at": [i.x, i.z], "reason": why}));
            false
        })
        .map(|i| {
            let mut v = json!({"asset": i.asset_id, "at": [i.x, i.z], "rot": i.rot.rem_euclid(360) / 90 * 90});
            if !i.tint.is_empty() {
                v["tint"] = i.tint.iter().map(|t| (t.key.clone(), json!(t.value))).collect::<Map<_, _>>().into();
            }
            v
        })
        .collect();
    let g = &plan.ground;
    let c = |v: i32| v.clamp(0, lib::GRID - 1);
    let scene_json = json!({
        "source": file_name,
        "ground": {"slab": [c(g.x0), c(g.x1), c(g.z0), c(g.z1)], "color": g.color},
        "instances": instances,
    });
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    fs::write(
        dir.join("scene.json"),
        serde_json::to_string_pretty(&scene_json).unwrap() + "\n",
    )
    .map_err(|e| e.to_string())?;
    let ext = file_name.rsplit('.').next().unwrap_or("png");
    fs::write(dir.join(format!("ref.{ext}")), image).map_err(|e| e.to_string())?;
    Ok(json!({
        "scene": scene,
        "objects": plan.objects.iter().map(|o| json!({"name": o.name, "match": o.kind, "asset_id": o.asset_id})).collect::<Vec<_>>(),
        "created_pending": created,
        "manual": manual,
        "dropped": dropped,
        "instances": instances.len(),
        "notes": plan.notes,
    }))
}

fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn system_prompt(library: &[Value]) -> String {
    format!(
        "Você converte imagens isométricas (pixel art, sprites) em cenas voxel montadas a partir de uma biblioteca de assets.\n\
A imagem não tem profundidade: você decide o layout 3D.\n\n\
Regras fixas:\n\
- Grade da cena 128x128 em x e z (0..127), altura até 127. Y para cima. Medidas em voxels de design. O chão é uma placa; objetos apoiam sobre ela.\n\
- Câmera iso: azimute 45°, elevação 30°. Na tela: esquerda = (x mín, z máx), topo = (x mín, z mín), direita = (x máx, z mín), baixo = (x máx, z máx).\n\
- Origem de cada asset = centro da base. Instância em (x, z) = centro da base na grade. Rotação só 0/90/180/270.\n\
- Fidelidade antes de tamanho: use quantos voxels forem precisos para ficar o mais fiel possível à imagem (ocupe a grade). Pessoa ≈ 12-16 voxels de altura, casa ≈ 32-48. Prefira rockpile com várias rochas a uma peça simples quando a imagem tiver rocha.\n\n\
Processo:\n\
1. Liste cada tipo de objeto visível.\n\
2. Para cada um, consulte a biblioteca: match=\"existing\" (use o id), \"variant\" (use o id existente e mude cor via tint na instância) ou \"new\".\n\
3. Objetos new: escolha uma peça e parâmetros. asset_id em kebab-case, minúsculas, terminando em -01. Se nenhuma peça representar bem, use piece=\"\" (será feito à mão).\n\
4. Posicione todas as instâncias (uma por objeto visível, inclusive repetidos) sem sobreposição.\n\
5. Cores em #rrggbb tiradas da imagem.\n\
Para existing/variant deixe piece, params, palette, tags e rocks vazios (rocks só em rockpile).\n\n\
Peças disponíveis (parâmetros em voxels; chaves de cor da paleta):\n\
- box: w, h, d — cor \"box\"\n\
- cyl: r, h — cor \"cyl\"\n\
- ball: r — cor \"ball\"\n\
- barrel: r, h — cores \"barrel\", \"barrelD\", \"hoop\"\n\
- oak: carvalho grande pronto (placa 18x18, copa ~39 de largura e ~37 de altura, com placa de chão própria; tile 0 tira a placa) — sem cores\n\
- crag: montanha de pedra pronta (~32x32 de base, com placa própria; tile 0 tira a placa) — sem cores\n\
- rockpile: montanha/rochedo como pilha de rochas; size (lado da placa, padrão 32), tile; em rocks liste cada rocha (x, z dentro da placa, r raio, h altura, e perfil 0.8, dome 1 para pedra arredondada, lit 1 para clarear) — sem cores\n\n\
Biblioteca atual (JSON):\n{}\n\n\
Em notes, diga em português o que ficou de fora ou precisa de peça nova.",
        serde_json::to_string(library).unwrap_or_default(),
    )
}

fn schema() -> Value {
    let kv = |t: &str| {
        json!({"type": "object", "properties": {"key": {"type": "string"}, "value": {"type": t}},
        "required": ["key", "value"], "additionalProperties": false})
    };
    json!({
        "type": "object",
        "properties": {
            "objects": {"type": "array", "items": {
                "type": "object",
                "properties": {
                    "name": {"type": "string"},
                    "match": {"type": "string", "enum": ["existing", "variant", "new"]},
                    "asset_id": {"type": "string"},
                    "piece": {"type": "string", "enum": ["", "box", "cyl", "ball", "barrel", "oak", "crag", "rockpile"]},
                    "params": {"type": "array", "items": kv("number")},
                    "palette": {"type": "array", "items": kv("string")},
                    "tags": {"type": "array", "items": {"type": "string"}},
                    "rocks": {"type": "array", "items": {"type": "object",
                        "properties": {"x": {"type": "number"}, "z": {"type": "number"}, "r": {"type": "number"}, "h": {"type": "number"},
                            "e": {"type": "number"}, "dome": {"type": "integer"}, "lit": {"type": "integer"}},
                        "required": ["x", "z", "r", "h", "e", "dome", "lit"], "additionalProperties": false}}
                },
                "required": ["name", "match", "asset_id", "piece", "params", "palette", "tags", "rocks"],
                "additionalProperties": false
            }},
            "ground": {"type": "object",
                "properties": {"x0": {"type": "integer"}, "x1": {"type": "integer"}, "z0": {"type": "integer"}, "z1": {"type": "integer"}, "color": {"type": "string"}},
                "required": ["x0", "x1", "z0", "z1", "color"], "additionalProperties": false},
            "instances": {"type": "array", "items": {
                "type": "object",
                "properties": {
                    "asset_id": {"type": "string"}, "x": {"type": "integer"}, "z": {"type": "integer"},
                    "rot": {"type": "integer", "enum": [0, 90, 180, 270]},
                    "tint": {"type": "array", "items": kv("string")}
                },
                "required": ["asset_id", "x", "z", "rot", "tint"], "additionalProperties": false
            }},
            "notes": {"type": "string"}
        },
        "required": ["objects", "ground", "instances", "notes"],
        "additionalProperties": false
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_pieces_match_library() {
        let s = schema();
        let e = &s["properties"]["objects"]["items"]["properties"]["piece"]["enum"];
        for p in pieces::NAMES {
            assert!(
                e.as_array().unwrap().contains(&json!(p)),
                "{p} faltando no schema"
            );
        }
    }

    #[test]
    fn apply_creates_pending_assets_and_scene() {
        let root = std::env::temp_dir().join(format!("voxel-analyze-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let plan: Plan = serde_json::from_value(json!({
            "objects": [
                {"name": "caixa", "match": "new", "asset_id": "crate-01", "piece": "box",
                 "params": [{"key": "w", "value": 4}], "palette": [{"key": "box", "value": "#aa8855"}], "tags": ["prop"]},
                {"name": "estátua", "match": "new", "asset_id": "statue-01", "piece": "", "params": [], "palette": [], "tags": []}
            ],
            "ground": {"x0": 0, "x1": 31, "z0": 0, "z1": 31, "color": "#6f9a4b"},
            "instances": [
                {"asset_id": "crate-01", "x": 5, "z": 6, "rot": 90, "tint": []},
                {"asset_id": "statue-01", "x": 9, "z": 9, "rot": 0, "tint": []},
                {"asset_id": "crate-01", "x": 200, "z": 9, "rot": 0, "tint": []}
            ],
            "notes": "ok"
        }))
        .unwrap();
        let dir = root.join("scenes").join("t");
        let out = apply(&root, plan, b"img", "x.png", "t", &dir).unwrap();
        assert_eq!(out["dropped"].as_array().unwrap().len(), 2);
        assert_eq!(out["created_pending"], json!(["crate-01"]));
        assert_eq!(out["manual"][0]["asset_id"], "statue-01");
        assert_eq!(out["instances"], 1);
        let a = lib::load_asset(&root, "crate-01").unwrap();
        assert_eq!(a.status, "pending");
        // Cena não monta até o asset ser aprovado.
        assert!(lib::scene_grid(&root, "t").is_err());
        lib::review(&root, "crate-01", "approve", None).unwrap();
        assert!(!lib::scene_grid(&root, "t").unwrap().vox.is_empty());
        fs::remove_dir_all(&root).unwrap();
    }
}
