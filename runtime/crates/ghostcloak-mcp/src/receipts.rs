//! Universal action receipts (Phase 0 of the agent-native runtime):
//! every mutation returns evidence, not hope. An action receipt is
//! `Act -> Observe -> Compare` composed from a11y snapshots the runtime
//! already has in flight:
//!
//!   before  = the page's cached a11y snapshot (agents run page_a11y /
//!             page_extract before acting — the playbook's own rule)
//!   after   = a fresh snapshot taken immediately after the mutation
//!   changes = ref-diff between the two (deterministic walk order makes
//!             refs stable across snapshots for unchanged elements)
//!
//! This is the runtime-level seed of the Agent.* receipt the C++ arc
//! grows into: same shape, deeper source later.

use ghostcloak_core::engine::{A11yElement, A11ySnapshot};

/// Compact element view for diff entries.
fn el_view(el: &A11yElement) -> serde_json::Value {
    serde_json::json!({
        "ref": el.r#ref,
        "role": el.role,
        "name": el.name,
    })
}

/// Diff two snapshots into the change format the agent protocol uses:
/// [{op: "add"|"remove"|"update", ref, role, name, value?, prev?}].
/// Refs come from the deterministic DOM walk, so an unchanged element
/// keeps its ref between snapshots — update detection is ref-keyed.
///
/// Output is capped: at most `cap` entries plus a truncated flag, so a
/// re-rendering SPA can't flood the agent's context.
pub fn diff(before: &A11ySnapshot, after: &A11ySnapshot, cap: usize) -> serde_json::Value {
    use std::collections::HashMap;
    let b: HashMap<&str, &A11yElement> = before
        .elements
        .iter()
        .map(|e| (e.r#ref.as_str(), e))
        .collect();
    let a: HashMap<&str, &A11yElement> = after
        .elements
        .iter()
        .map(|e| (e.r#ref.as_str(), e))
        .collect();

    let mut changes: Vec<serde_json::Value> = Vec::new();
    let mut added = 0usize;
    let mut removed = 0usize;
    let mut updated = 0usize;

    for (r, el) in &a {
        match b.get(r) {
            None => added += 1,
            Some(prev) => {
                if el.value != prev.value
                    || el.name != prev.name
                    || el.checked != prev.checked
                    || el.disabled != prev.disabled
                    || el.expanded != prev.expanded
                {
                    updated += 1;
                }
            }
        }
    }
    for r in b.keys() {
        if !a.contains_key(r) {
            removed += 1;
        }
    }

    if added + removed + updated == 0 {
        return serde_json::json!({ "changed": false });
    }

    for (r, el) in &a {
        if changes.len() >= cap {
            break;
        }
        match b.get(r) {
            None => {
                if added > 0 {
                    let mut v = el_view(el);
                    v["op"] = "add".into();
                    changes.push(v);
                }
            }
            Some(prev) => {
                if (el.value != prev.value
                    || el.name != prev.name
                    || el.checked != prev.checked
                    || el.disabled != prev.disabled
                    || el.expanded != prev.expanded)
                    && changes.len() < cap
                {
                    let mut v = el_view(el);
                    v["op"] = "update".into();
                    v["value"] = serde_json::to_value(&el.value).unwrap_or_default();
                    v["prev"] = serde_json::to_value(&prev.value).unwrap_or_default();
                    changes.push(v);
                }
            }
        }
    }
    for (r, el) in &b {
        if changes.len() >= cap {
            break;
        }
        if !a.contains_key(r) {
            let mut v = el_view(el);
            v["op"] = "remove".into();
            let _ = r;
            changes.push(v);
        }
    }

    serde_json::json!({
        "changed": true,
        "adds": added,
        "removes": removed,
        "updates": updated,
        "truncated": added + removed + updated > changes.len(),
        "changes": changes,
    })
}
