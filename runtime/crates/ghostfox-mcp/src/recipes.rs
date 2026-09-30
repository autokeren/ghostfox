//! Deterministic action recipes: record a successful tool flow, replay it
//! later without an LLM in the loop. Recorded steps carry SEMANTIC anchors
//! (role + accessible name) instead of ephemeral refs, so a replay survives
//! DOM churn: anchors re-resolve against a fresh a11y snapshot, and a
//! missing anchor escalates (strict mode) instead of clicking blindly.
//!
//! Storage: ~/.ghostfox/recipes/<name>.json (GHOSTFOX_RECIPES to override).

use std::path::{Path, PathBuf};

/// Semantic anchor for ref-based actions, captured at record time from the
/// last a11y snapshot the caller had. (role, name) pairs re-resolve on
/// replay the same way page_extract filters do.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Anchor {
    pub role: String,
    pub name: String,
}

/// One recorded action. `params` mirrors the original tool arguments
/// (url, text, key, selector, ...); `anchor` is present for `*_ref` tools.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecipeStep {
    pub tool: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<Anchor>,
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Recipe {
    pub name: String,
    pub created_at: String,
    pub steps: Vec<RecipeStep>,
}

fn recipes_root() -> PathBuf {
    std::env::var_os("GHOSTFOX_RECIPES")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            Path::new(&home).join(".ghostfox").join("recipes")
        })
}

fn safe_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(64)
        .collect()
}

pub fn save(recipe: &Recipe) -> Result<PathBuf, String> {
    let dir = recipes_root();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.json", safe_name(&recipe.name)));
    let tmp = dir.join(format!(".{}.tmp", safe_name(&recipe.name)));
    std::fs::write(
        &tmp,
        serde_json::to_string_pretty(recipe).unwrap_or_default(),
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn load(name: &str) -> Result<Recipe, String> {
    let path = recipes_root().join(format!("{}.json", safe_name(name)));
    if !path.exists() {
        return Err(format!("recipe `{name}` not found — call recipe_list"));
    }
    let body = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&body).map_err(|e| format!("recipe `{name}` corrupt: {e}"))
}

pub fn list() -> Vec<Recipe> {
    let dir = recipes_root();
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            if e.path().extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            if let Ok(body) = std::fs::read_to_string(e.path()) {
                if let Ok(r) = serde_json::from_str::<Recipe>(&body) {
                    out.push(r);
                }
            }
        }
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    out
}

/// Resolve an anchor against a snapshot: exact role (case-insensitive),
/// substring name (case-insensitive), first match in DOM order.
pub fn resolve_anchor<'a>(
    snap: &'a ghostfox_core::engine::A11ySnapshot,
    anchor: &Anchor,
) -> Option<&'a ghostfox_core::engine::A11yElement> {
    snap.elements.iter().find(|el| {
        el.role.eq_ignore_ascii_case(&anchor.role)
            && el.name.to_lowercase().contains(&anchor.name.to_lowercase())
    })
}

/// Resolve the anchor behind a recorded ref (captured at record time) from
/// a live snapshot — used by the note hook in the MCP layer.
pub fn anchor_from_ref(
    snap: &ghostfox_core::engine::A11ySnapshot,
    r#ref: &str,
) -> Option<Anchor> {
    snap.elements
        .iter()
        .find(|el| el.r#ref == r#ref)
        .map(|el| Anchor {
            role: el.role.clone(),
            name: el.name.clone(),
        })
}
