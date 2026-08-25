# quilt-vm-wasm

The 5-opcode Quilt VM in WASM. **Layer 1 of the polyformalism:**
the substrate compiled to the web.

> A runtime is a function from context to value with an inverse,
> advanced by a clock that processes async I/O while projecting
> a sync view.

## What this is

The 5 opcodes (BIND, LINK, EFFECT, VIEW, TICK) as a WASM library,
built with wasm-bindgen. The same opcodes that exist in Python,
C, Rust, TypeScript, and Haskell exist here as JavaScript-callable
functions.

## The 5 opcodes

```rust
vm.bind("bathy:0", serde_json::json!(4.2));     // BIND — make a thing
vm.link("a", "b", "depends_on");                // LINK — connect things
vm.effect("counter", "inc", "dec");             // EFFECT — reversible change
vm.view("bathy:0", "anyone");                  // VIEW — project for viewer
vm.tick(1.0);                                  // TICK — advance time
```

In JavaScript, the same:

```javascript
const vm = new WasmQuiltVM();
vm.bind("bathy:0", 4.2);
vm.link("bathy:0", "tide:current", "depends_on");
const v = vm.view("bathy:0", "anyone");
vm.tick(1.0);
```

## Build

```bash
# Native (for testing)
cargo test

# WASM (for the browser)
wasm-pack build --target web
```

## The browser demo

After `wasm-pack build`, open `www/index.html` in a browser.
Click "Run Gold Demo" to see all 8 polyformalisms execute.

## Tests

5 tests in `src/lib.rs` — bind/view, link/reachable, effect, tick,
and the gold demo.

## Why WASM

WASM is the cross-platform bytecode. The 5 opcodes that work in
C work in WASM, work in JavaScript, work in Rust, work in any
host language. The substrate survives the layer change.

## Version

0.1.0 — first public release.
