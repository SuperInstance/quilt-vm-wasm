//! quilt-vm-wasm — Layer 1 of the polyformalism.
//!
//! The 5 opcodes (BIND, LINK, EFFECT, VIEW, TICK) as a WASM library.
//! The substrate compiled to the web. The same opcodes that exist
//! in Python, C, Rust, TypeScript, and Haskell exist here as
//! wasm-bindgen-exported JavaScript functions.
//!
//! On native targets, the crate compiles to a normal Rust library
//! (for testing). On `wasm32-unknown-unknown`, the `bindgen` module
//! re-exports the opcodes as JavaScript-callable functions.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

// === The 5 opcodes as data types ===

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Cell {
    pub name: String,
    pub value: serde_json::Value,
    pub immutable: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Link {
    pub a: String,
    pub b: String,
    pub relation: String,
    pub weight: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EffectRecord {
    pub target: String,
    pub forward_name: String,
    pub inverse_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ViewRecord {
    pub target: String,
    pub viewer: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TickRecord {
    pub dt: f64,
    pub time: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct QuiltVM {
    pub cells: BTreeMap<String, Cell>,
    pub links: Vec<Link>,
    pub effects: Vec<EffectRecord>,
    pub views: Vec<ViewRecord>,
    pub ticks: Vec<TickRecord>,
    pub time: f64,
}

impl QuiltVM {
    pub fn new() -> Self { Self::default() }

    /// Opcode 1: BIND
    pub fn bind(&mut self, name: &str, value: serde_json::Value) -> &Cell {
        let cell = Cell { name: name.to_string(), value, immutable: true };
        self.cells.insert(name.to_string(), cell);
        self.cells.get(name).unwrap()
    }

    /// Opcode 2: LINK
    pub fn link(&mut self, a: &str, b: &str, relation: &str) {
        self.links.push(Link {
            a: a.to_string(), b: b.to_string(),
            relation: relation.to_string(), weight: 1.0,
        });
    }

    /// Opcode 3: EFFECT
    pub fn effect(&mut self, target: &str, forward: &str, inverse: &str) {
        self.effects.push(EffectRecord {
            target: target.to_string(),
            forward_name: forward.to_string(),
            inverse_name: inverse.to_string(),
        });
    }

    /// Opcode 4: VIEW
    pub fn view(&mut self, target: &str, viewer: &str) -> Option<&Cell> {
        self.views.push(ViewRecord { target: target.to_string(), viewer: viewer.to_string() });
        self.cells.get(target)
    }

    /// Opcode 5: TICK
    pub fn tick(&mut self, dt: f64) {
        self.time += dt;
        self.ticks.push(TickRecord { dt, time: self.time });
    }

    pub fn stats(&self) -> serde_json::Value {
        serde_json::json!({
            "n_cells": self.cells.len(),
            "n_links": self.links.len(),
            "n_effects": self.effects.len(),
            "n_views": self.views.len(),
            "n_ticks": self.ticks.len(),
            "time": self.time,
        })
    }

    pub fn reachable(&self, start: &str, relation: Option<&str>) -> Vec<String> {
        let rel = relation.unwrap_or("");
        let adj: HashMap<&str, Vec<&str>> = self.links.iter()
            .filter(|l| rel.is_empty() || l.relation == rel)
            .fold(HashMap::new(), |mut m, l| {
                m.entry(l.a.as_str()).or_default().push(l.b.as_str());
                m
            });
        let mut seen = BTreeSet::new();
        let mut stack = vec![start];
        while let Some(cur) = stack.pop() {
            if !seen.insert(cur.to_string()) { continue; }
            if let Some(nexts) = adj.get(cur) {
                for n in nexts { stack.push(n); }
            }
        }
        seen.into_iter().collect()
    }
}

// === Tests ===

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bind_and_view() {
        let mut vm = QuiltVM::new();
        vm.bind("bathy:0", serde_json::json!(4.2));
        let v = vm.view("bathy:0", "anyone").unwrap();
        assert_eq!(v.value, serde_json::json!(4.2));
    }

    #[test]
    fn link_and_reachable() {
        let mut vm = QuiltVM::new();
        vm.bind("a", serde_json::json!(1));
        vm.bind("b", serde_json::json!(2));
        vm.link("a", "b", "depends_on");
        let r = vm.reachable("a", Some("depends_on"));
        assert!(r.contains(&"a".to_string()));
        assert!(r.contains(&"b".to_string()));
    }

    #[test]
    fn effect_record() {
        let mut vm = QuiltVM::new();
        vm.bind("counter", serde_json::json!(0));
        vm.effect("counter", "inc", "dec");
        assert_eq!(vm.effects.len(), 1);
    }

    #[test]
    fn tick_advances_time() {
        let mut vm = QuiltVM::new();
        vm.tick(1.0);
        assert_eq!(vm.time, 1.0);
        vm.tick(2.5);
        assert_eq!(vm.time, 3.5);
    }

    #[test]
    fn gold_demo() {
        let mut vm = QuiltVM::new();
        vm.bind("bathy:0", serde_json::json!(4.2));
        vm.bind("tide:current", serde_json::json!(0));
        vm.link("bathy:0", "tide:current", "depends_on");
        vm.view("bathy:0", "anyone");
        vm.tick(1.0);
        let stats = vm.stats();
        assert_eq!(stats["n_cells"], 2);
        assert_eq!(stats["n_links"], 1);
        assert_eq!(stats["n_views"], 1);
        assert_eq!(stats["time"], 1.0);
    }
}

// === WASM bindings ===

#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
mod wasm_bindings {
    use super::*;
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    pub struct WasmQuiltVM(QuiltVM);

    #[wasm_bindgen]
    impl WasmQuiltVM {
        #[wasm_bindgen(constructor)]
        pub fn new() -> Self { Self(QuiltVM::new()) }

        #[wasm_bindgen]
        pub fn bind(&mut self, name: &str, value: JsValue) -> Result<(), JsValue> {
            let v: serde_json::Value = serde_wasm_bindgen::from_value(value)?;
            self.0.bind(name, v);
            Ok(())
        }

        #[wasm_bindgen]
        pub fn link(&mut self, a: &str, b: &str, relation: &str) {
            self.0.link(a, b, relation);
        }

        #[wasm_bindgen]
        pub fn effect(&mut self, target: &str, forward: &str, inverse: &str) {
            self.0.effect(target, forward, inverse);
        }

        #[wasm_bindgen]
        pub fn view(&mut self, target: &str, viewer: &str) -> JsValue {
            match self.0.view(target, viewer) {
                Some(c) => serde_wasm_bindgen::to_value(&c.value).unwrap(),
                None => JsValue::NULL,
            }
        }

        #[wasm_bindgen]
        pub fn tick(&mut self, dt: f64) {
            self.0.tick(dt);
        }

        /// Direct clock readback. stats() can't carry `time` to JS until the
        /// serde-wasm-bindgen object-serialization bug is fixed (it returns
        /// empty objects), so scalar getters are the honest path.
        #[wasm_bindgen]
        pub fn time(&self) -> f64 {
            self.0.time
        }

        #[wasm_bindgen]
        pub fn stats(&self) -> JsValue {
            serde_wasm_bindgen::to_value(&self.0.stats()).unwrap()
        }

        #[wasm_bindgen]
        pub fn reachable(&self, start: &str, relation: &str) -> JsValue {
            let r = self.0.reachable(start, if relation.is_empty() { None } else { Some(relation) });
            serde_wasm_bindgen::to_value(&r).unwrap()
        }
    }
}
