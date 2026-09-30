# Blueprint: build the quilt VM to WASM and run it from JavaScript

## 1. In one breath

Compile `src/lib.rs` to `wasm32-unknown-unknown` with the `wasm` feature on, generate JS glue with `wasm-bindgen`, and call `WasmQuiltVM` from Node or a browser.

## 2. Why it exists

The crate is useful to a JS host only after this pipeline runs, and the README's shortcut (`wasm-pack build --target web`) omits a required step: the exported class lives behind the `wasm` Cargo feature, which is off by default. Without it you get a valid, loadable package that exports no `WasmQuiltVM` at all. This page gives the sequence I ran, the receipt for it, and the traps.

## 3. The mental model

Four things, in a line: **Rust source → `.wasm` binary → JS glue → host script.**

- **Feature `wasm`** (`Cargo.toml`): switches on `#[cfg(all(target_arch = "wasm32", feature = "wasm"))] mod wasm_bindings` in `src/lib.rs`, the only place `#[wasm_bindgen]` appears. On native targets the module is compiled out, so `cargo test` never touches it.
- **The `.wasm` binary** (`target/wasm32-unknown-unknown/<profile>/quilt_vm_wasm.wasm`): raw output of `cargo build`. It contains wasm-bindgen's descriptors but is not directly callable with nice types.
- **The glue** (`wasm-bindgen` CLI output): a `.js` file, a `_bg.wasm`, and `.d.ts` files. The `--target` flag picks the flavour: `nodejs` (CommonJS, loads synchronously) or `web` (ES module, needs `await init()`).
- **The host**: your script. It sees `WasmQuiltVM` with `bind/link/effect/view/tick/time/stats/reachable`. Values cross the boundary as JSON-shaped data via `serde-wasm-bindgen`; JSON objects come back as `Map`s.

The CLI version must match the `wasm-bindgen` crate version in `Cargo.lock`. Here that was 0.2.129.

## 4. Walkthrough

Prerequisites I had to add in this environment: the wasm target and the bindgen CLI. (`wasm-pack` is not installed here and I did not use it; this is the equivalent manual pipeline.)

```console
$ rustup target add wasm32-unknown-unknown
$ cargo install wasm-bindgen-cli --version 0.2.129
```

Check which version you need: `grep -A1 'name = "wasm-bindgen"' Cargo.lock`.

**Build the binary with the feature on:**

```console
$ cargo build --release --target wasm32-unknown-unknown --features wasm
    Finished `release` profile [optimized] target(s) ...
```

**Generate glue for Node** (use `--target web` for a browser):

```console
$ wasm-bindgen --target nodejs --out-dir pkg \
    target/wasm32-unknown-unknown/release/quilt_vm_wasm.wasm
```

Release output for `--target web` was `quilt_vm_wasm_bg.wasm` at 134,928 bytes plus a 21,288-byte `quilt_vm_wasm.js`. Write `pkg/` somewhere ignored (I used a scratch directory outside the repo; the repo has no `.gitignore`).

**Run it.** `smoke.js` (in `pkg`'s parent directory):

```js
const assert = require('assert');
const { WasmQuiltVM } = require('./pkg/quilt_vm_wasm.js');
const vm = new WasmQuiltVM();
vm.bind("bathy:0", 4.2);
vm.bind("tide:current", 0);
vm.link("bathy:0", "tide:current", "depends_on");
assert.strictEqual(vm.view("bathy:0", "anyone"), 4.2);
vm.tick(1.0);
const s = Object.fromEntries(vm.stats());   // stats() returns a Map
assert.deepStrictEqual(s, { n_cells:2, n_effects:0, n_links:1, n_ticks:1, n_views:1, time:1 });
assert.deepStrictEqual(vm.reachable("bathy:0","depends_on"), ["bathy:0","tide:current"]);
console.log("smoke ok", JSON.stringify(s));
```

```console
$ node smoke.js
smoke ok {"n_cells":2,"n_effects":0,"n_links":1,"n_ticks":1,"n_views":1,"time":1}
```

(My run used `./pkgrel/` as the output directory name for the release build; the content is identical to the `pkg/` shown above.)

**Browser variant.** Generate with `--target web`, then load as an ES module:

```js
import init, { WasmQuiltVM } from './quilt_vm_wasm.js';
await init();
const vm = new WasmQuiltVM();
```

I generated the `web` glue and confirmed it exports `WasmQuiltVM` (5 occurrences of the name in the JS), but did **not** execute it in a browser. `www/index.html` uses exactly this import with the path `./quilt_vm_wasm.js`, so copy the three generated files next to it.

## 5. The contract

**Inputs:** the repo checkout; Rust with the `wasm32-unknown-unknown` target; `wasm-bindgen-cli` whose version equals the locked `wasm-bindgen` crate.

**Outputs:** a `.wasm` plus glue exposing one class, `WasmQuiltVM`, with methods:

| Method | Returns |
|---|---|
| `new WasmQuiltVM()` | instance |
| `bind(name, value)` | nothing; throws if `value` isn't convertible |
| `link(a, b, relation)` | nothing |
| `effect(target, forward, inverse)` | nothing (records only) |
| `view(target, viewer)` | the value, or `null` if unbound |
| `tick(dt)` | nothing |
| `time()` | number |
| `stats()` | `Map` of counts and `time` |
| `reachable(start, relation)` | `string[]`; pass `""` for any relation |

**Invariants:** the exports exist iff the `wasm` feature was on at build time; instances are independent.

**Receipts (all run in this checkout):**
- `cargo test` → `test result: ok. 5 passed; 0 failed` (native only; the bindings are not covered).
- `cargo build --target wasm32-unknown-unknown --features wasm` → success (debug and release).
- `node smoke.js` → `smoke ok {...}` with all three assertions passing. That script is the only automated check of the WASM path, and it is not committed to the repo; copy it from above.
- Feature-gate check: `wasm-bindgen --target web` on a build *without* `--features wasm` gave a `quilt_vm_wasm.js` with 0 occurrences of `WasmQuiltVM`; with the feature, 5.

## 6. Failure modes / scars

- **Missing target.** `error[E0463]: can't find crate for 'core'` while compiling `unicode-ident`, with the hint `rustup target add wasm32-unknown-unknown`. Fix: add the target.
- **Silent empty package.** No feature → no `WasmQuiltVM`; the failure only shows at runtime (`WasmQuiltVM is not a constructor`, or an import error in a browser). Fix: `--features wasm`; with wasm-pack, `wasm-pack build --target web -- --features wasm` (not run here).
- **Version skew.** `wasm-bindgen` CLI and crate must match, otherwise the CLI rejects the module with a schema-version mismatch. Fix: install the CLI at the version in `Cargo.lock`.
- **Wrong `--target` for the host.** `nodejs` glue uses `require` and loads the wasm synchronously; `web` glue needs `await init()` and `fetch`. Mixing them fails at import.
- **`Map` instead of object.** Wrap `stats()` with `Object.fromEntries`; `JSON.stringify(map)` gives `{}`. This also affects object-valued cells returned by `view`.
- **`www/index.html` path.** The page imports `./quilt_vm_wasm.js` from its own directory, while wasm-pack emits to `pkg/`. Copy or adjust; the page also `JSON.stringify`s a `Map`, so its `stats` field will not display as intended (untested in a browser).
- **Nothing runs on effect/tick.** If your integration expects `tick` to execute registered effects, it will not; see the semantics table in [understanding-quilt-vm-wasm.md](understanding-quilt-vm-wasm.md).

## 7. How it composes

- **CI:** the steps are non-interactive and could run as `cargo test` then the build, bindgen, and `node smoke.js`. There is no CI config in the repo today.
- **Bundlers/edge runtimes:** `--target bundler` and `--target deno` are other wasm-bindgen outputs; I did not try them.
- **Extending the API:** new methods go in `wasm_bindings` in `src/lib.rs`; any addition should also get a line in the smoke script, since native tests cannot see it.
- **`wasm-bindgen-test`** is already a dev-dependency (`Cargo.toml`) but no test uses it. `wasm-bindgen-test-runner` was installed by the CLI package above, so browser/Node-based Rust tests of the bindings are a plausible next step; I have not tried it.

## 8. Where to look next

- [`Cargo.toml`](../Cargo.toml): the `wasm` feature and the target-specific dependencies.
- [`src/lib.rs`](../src/lib.rs) (`mod wasm_bindings`, bottom of file): the exact exported surface.
- [understanding-quilt-vm-wasm.md](understanding-quilt-vm-wasm.md): what the exported operations actually do.
