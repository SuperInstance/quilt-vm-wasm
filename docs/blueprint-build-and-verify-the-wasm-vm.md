# Blueprint: build and verify the WASM VM

Every command in §4 was run at commit `001fa53`. Steps that need `wasm-pack` or a browser were **not** run and are marked.

## 1. In one breath

Prove three things in order: the VM logic passes its tests natively, it compiles to `wasm32` with the JS-facing class enabled, and the resulting `.wasm` actually exports that class — before you spend time on browser glue.

## 2. Why it exists

The repo has one file of logic and two build modes. The most common mistake is building the wasm without `--features wasm`, which succeeds and produces a module with no VM in it. This workflow catches that early and cheaply, and gives a place to test logic changes without a browser.

## 3. The mental model

- **Native build**: normal Rust; runs `cargo test`. Contains `QuiltVM` only.
- **Feature `wasm`**: an opt-in in `Cargo.toml` (`wasm = ["dep:wasm-bindgen"]`) that, on `wasm32` only, compiles `wasm_bindings::WasmQuiltVM`.
- **Raw `.wasm`**: what `cargo build --target wasm32-unknown-unknown` emits in `target/wasm32-unknown-unknown/release/`. Needs `wasm-bindgen` post-processing to become usable from JS.
- **Glue (`pkg/`)**: the JS + processed wasm from `wasm-pack`; `www/index.html` expects `./quilt_vm_wasm.js`.

## 4. Walkthrough

**Prereqs**: `rustup target add wasm32-unknown-unknown`, Node (for the export check).

**Step 1 — native tests**

```
$ cargo test
running 5 tests
test tests::effect_record ... ok
test tests::bind_and_view ... ok
test tests::gold_demo ... ok
test tests::tick_advances_time ... ok
test tests::link_and_reachable ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Doc-tests: 0. Note `wasm-bindgen-test` is a dev-dependency but no `#[wasm_bindgen_test]` exists, so the bindings have **no tests**.

**Step 2 — compile the bindings**

```
$ cargo check --features wasm         # native: fast, but bindings are cfg'd out (wasm32 only)
$ cargo build --target wasm32-unknown-unknown --features wasm --release
    Finished `release` profile [optimized] target(s)
```

`cargo check --features wasm` on the native target passes without compiling the bindings at all; only the wasm32 build checks them.

**Step 3 — verify the exports**

```
$ node -e '
const b=require("fs").readFileSync("target/wasm32-unknown-unknown/release/quilt_vm_wasm.wasm");
const m=new WebAssembly.Module(b);
console.log(WebAssembly.Module.exports(m).map(e=>e.name).filter(n=>/^wasmquiltvm_|^__wbg_wasmquiltvm/.test(n)).join("\n"))'
```

This run's output lists (hash suffix elided here): `__wbg_wasmquiltvm_free`, `wasmquiltvm_bind`, `_effect`, `_link`, `_new`, `_reachable`, `_stats`, `_tick`, `_time`, `_view`. Nine methods plus `free` — matching the impl block. The same build **without** `--features wasm` exported only `memory`, `__data_end`, `__heap_base`.

**Step 4 — exercise logic natively** (optional): a scratch crate with `quilt-vm-wasm = { path = "…" }` can call `QuiltVM` directly; see §4b of [understanding-quilt-vm-wasm.md](understanding-quilt-vm-wasm.md). For the fuller wasm-bindgen + Node run, see [blueprint-build-and-run-in-wasm.md](blueprint-build-and-run-in-wasm.md) (this blueprint stops at the raw `.wasm`, because `wasm-bindgen` was not installed when it was written).

**Step 5 — browser (not run here)**: per README, `wasm-pack build --target web -- --features wasm`, then get `pkg/*` next to `www/index.html` (it imports `./quilt_vm_wasm.js`), then `python3 -m http.server 8000 --directory www`. I did not run this; wasm-pack was unavailable.

## 5. The contract

- **Inputs**: a checkout, a Rust toolchain, the wasm32 target, Node for step 3.
- **Outputs**: test pass count (5), a `.wasm` file, its export list.
- **Invariants**: the `wasmquiltvm_*` export set equals the public methods of `WasmQuiltVM` (new, bind, link, effect, view, tick, time, stats, reachable + free). If you add a method and it is missing from the list, the feature or cfg gate is wrong.
- **Receipt**: the "5 passed" line and the export list. Nothing is signed. `cargo test` count is currently 5 (README also says 5).

## 6. Failure modes and scars

- Missing `--features wasm` → build succeeds, exports empty of VM (verified).
- `wasm-pack build` uses default features unless you pass them after `--` (from the wasm-pack design and the manifest; not run here).
- `www/` vs `pkg/` path mismatch (see understanding doc §6).
- `stats()` from JS may give empty objects (comment in `src/lib.rs`); use `time()` and check counts via `reachable`/your own bookkeeping. Unverified in a browser.
- Raw `.wasm` is ~590 KB in this unstripped release build (measured on the feature build; no `wasm-opt`). Size is informational, not a target.
- Build creates untracked `target/` and `Cargo.lock`; the repo has no `.gitignore`.
- Exports are hash-suffixed, so grep by prefix, not exact name.

## 7. How it composes

Steps 1–3 fit CI as-is (no browser). Step 5 is the integration check and the only piece that exercises real JS↔wasm conversion (`serde_wasm_bindgen`). A reasonable extension, not done here: add `#[wasm_bindgen_test]` cases and run them with `wasm-pack test --node`.

## 8. Where to look next

- [`Cargo.toml`](../Cargo.toml) — features, crate types, target-specific deps.
- [`src/lib.rs`](../src/lib.rs) — the `wasm_bindings` module at the bottom.
- [`www/index.html`](../www/index.html) — the demo the browser step should reproduce.
