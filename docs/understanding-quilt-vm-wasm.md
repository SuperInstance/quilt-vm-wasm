# Understanding quilt-vm-wasm

## 1. In one breath

quilt-vm-wasm is a ~250-line Rust library that keeps a named-value graph plus an append-only log of five operations (BIND, LINK, EFFECT, VIEW, TICK), and can be compiled to WebAssembly so JavaScript can call those five operations.

## 2. Why it exists

Before this, the five-opcode model described in the README existed in other languages (the README names C, Rust, Python, TypeScript, Haskell) but not somewhere a web page could call it without a server. This crate is the same small data model compiled for `wasm32-unknown-unknown`, with a thin `wasm-bindgen` wrapper (`WasmQuiltVM`) so a browser tab or Node process can hold a VM in memory and call `bind`, `link`, `effect`, `view`, `tick`.

It is a data recorder with one graph query, not an interpreter. Sections 3 and 6 spell out exactly what it does and does not do, because the README's prose describes more than the code implements.

## 3. The mental model

There is one noun that holds everything, `QuiltVM` (`src/lib.rs:52`), and it is a struct of five collections plus a clock:

| Field | Type | Written by | Meaning |
|---|---|---|---|
| `cells` | `BTreeMap<String, Cell>` | `bind` | Named values. A `Cell` is `{name, value: serde_json::Value, immutable}`. |
| `links` | `Vec<Link>` | `link` | Directed, typed edges `{a, b, relation, weight}`; `weight` is always `1.0`. |
| `effects` | `Vec<EffectRecord>` | `effect` | Registered `{target, forward_name, inverse_name}` triples. |
| `views` | `Vec<ViewRecord>` | `view` | A log of who looked at what. |
| `ticks` | `Vec<TickRecord>` | `tick` | A log of `{dt, time}`. |
| `time` | `f64` | `tick` | Running sum of every `dt`. |

Five verbs act on it. The important thing is which of them change *state* and which only *append a record*:

- **BIND(name, value)** replaces the cell at `name`. It is the only operation that creates or changes a value. Rebinding silently overwrites (see section 6).
- **LINK(a, b, relation)** appends an edge. It does not check that `a` or `b` are bound, and it does not de-duplicate.
- **EFFECT(target, forward, inverse)** appends a record of two *names*. Nothing is executed and the target's value is untouched. The "inverse" is a string, not a function.
- **VIEW(target, viewer)** appends a `ViewRecord` and returns the cell if it exists. The viewer string is logged but never consulted, so it does not filter or format anything.
- **TICK(dt)** adds `dt` to `time` and appends a record. It does not run effects, recompute views, or wake anything.

The one derived query is `reachable(start, relation)`: a depth-first walk over `links` from `start`, following edges of one relation (or all if `None`), returning the sorted set of visited names, always including `start` itself even if unbound.

So the honest model is: **a key-value store, an edge list, three audit logs, and a clock**. The README's "reversible effects", "access control" and "scheduler" are the intent that the record types leave room for; the code today stores the data such behaviour would need but does not implement the behaviour.

`stats()` summarises counts: `n_cells` (distinct names), `n_links`, `n_effects`, `n_views`, `n_ticks`, and `time`.

## 4. Walkthrough

Two runs: natively (no browser, no WASM tooling), then the compiled module under Node. Everything below was executed in this repo's checkout.

### 4a. Native: the built-in tests

```console
$ cargo test
running 5 tests
test tests::bind_and_view ... ok
test tests::link_and_reachable ... ok
test tests::gold_demo ... ok
test tests::tick_advances_time ... ok
test tests::effect_record ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### 4b. Native: probing behaviour the tests don't cover

I wrote a throwaway program outside the repo (a separate crate with a path dependency on this one) to check what the code really does:

```console
rebind -> 99, immutable=true
after effect+tick counter=99 effects=1
view missing is_none=true
views logged=1
cyclic depends_on from a: ["a", "b"]
any relation from a: ["a", "b", "c"]
unbound start: ["zzz"]
dangling link ok, links=3
roundtrip equal stats: true
{"n_cells":1,"n_effects":1,"n_links":3,"n_ticks":1,"n_views":1,"time":1.0}
```

Read against the code: rebinding `counter` replaced 0 with 99 even though the cell says `immutable=true`; registering an effect and ticking left the value at 99; a view of a missing cell returns nothing but is *still logged*; the cycle `a→b→a` terminates; and `QuiltVM` round-trips through serde JSON (it derives `Serialize`/`Deserialize`).

### 4c. Compiled to WASM, driven from Node

The build steps are in [blueprint-build-and-run-in-wasm.md](blueprint-build-and-run-in-wasm.md). With the glue generated, this script:

```js
const { WasmQuiltVM } = require('./pkg/quilt_vm_wasm.js');
const vm = new WasmQuiltVM();
vm.bind("bathy:0", 4.2);
vm.link("bathy:0", "tide:current", "depends_on");
console.log("view:", vm.view("bathy:0", "anyone"));
console.log("view missing:", vm.view("nope", "anyone"));
vm.tick(1.0); vm.tick(2.5);
console.log("time:", vm.time());
console.log("stats:", vm.stats());
console.log("reachable:", vm.reachable("bathy:0", "depends_on"));
vm.bind("player:gandalf", { perception: 15 });
console.log("object view:", vm.view("player:gandalf", "anyone"));
```

printed:

```console
view: 4.2
view missing: null
time: 3.5
stats: Map(6) {
  'n_cells' => 1,
  'n_effects' => 0,
  'n_links' => 1,
  'n_ticks' => 2,
  'n_views' => 2,
  'time' => 3.5
}
reachable: [ 'bathy:0', 'tide:current' ]
object view: Map(1) { 'perception' => 15 }
```

Note that JS receives `Map` objects, not plain objects, for anything that is a JSON object (`stats()` and object-valued cells). See section 6.

## 5. The contract

**Inputs.** Names are arbitrary strings. Values passed to `bind` in JS are converted with `serde_wasm_bindgen::from_value` into `serde_json::Value`; a value that cannot convert makes `bind` throw (it returns `Result<(), JsValue>`). `dt` is any `f64`; negative values are accepted and move `time` backwards.

**Outputs.** `view` returns the cell's value or `null`. `reachable` returns an array of strings (sorted, since it comes from a `BTreeSet`). `stats()` returns a `Map`. `time()` returns a number.

**Invariants the code actually guarantees.**
- `n_cells` counts distinct names; `n_links`, `n_effects`, `n_views`, `n_ticks` count calls (never de-duplicated).
- `time` equals the sum of all `dt` passed to `tick` (test `tick_advances_time`: 1.0 then 2.5 gives 3.5).
- `reachable` terminates on cycles and includes `start`.
- The whole VM state serializes to JSON and back.

**Not guaranteed** (despite README wording): immutability of cells, execution or reversal of effects, viewer-based access control, anything happening at tick time.

**Receipt.** `cargo test` in this checkout: **5 passed, 0 failed** (unit tests) and 0 doc-tests. `cargo build --target wasm32-unknown-unknown --features wasm` succeeds and produces `target/wasm32-unknown-unknown/debug/quilt_vm_wasm.wasm`. The tests cover only native code; the `wasm_bindings` module (`#[cfg(all(target_arch = "wasm32", feature = "wasm"))]`) has no automated tests. Its behaviour above was checked by hand under Node.

## 6. Failure modes / scars

- **`wasm-pack build --target web` (as in the README) produces a package with no `WasmQuiltVM`.** The bindings are gated on the `wasm` Cargo feature (`Cargo.toml`, `default = []`), and the README command doesn't enable it. Fix: pass the feature, e.g. `wasm-pack build --target web -- --features wasm`. I did not run `wasm-pack` itself (it isn't installed here); I verified the feature gate by building with `--features wasm` and running the resulting module through `wasm-bindgen`.
- **`www/index.html` imports `./quilt_vm_wasm.js`, but `wasm-pack` writes to `pkg/`.** Serving `www/` as the README says will 404 on the import unless you copy the `pkg/` files next to the page (or change the import path). Not tested in a browser here.
- **Objects arrive in JS as `Map`, not plain objects.** `serde_json::Value` objects are serialized by `serde-wasm-bindgen` as `Map` by default. The code comment on `time()` blames an "empty objects" bug in `stats()`; under this build (wasm-bindgen 0.2.129, serde-wasm-bindgen 0.6.5) `stats()` returned a populated `Map`, so what you see is Map-vs-object, which `JSON.stringify` renders as `{}`. That is likely what `www/index.html` hits when it stringifies `stats`. Fix on the JS side: `Object.fromEntries(vm.stats())`, or use `vm.time()` for the clock. Changing the Rust serializer is a code change and out of scope here.
- **`immutable: true` is a label, not a rule.** `bind` on an existing name overwrites. Anyone relying on immutability must check before binding.
- **EFFECT does nothing at runtime.** The README's `inc`/`dec` example registers names; the counter never changes. `www/index.html` even registers an effect on `counter` without ever binding it, and that is accepted.
- **VIEW's viewer is ignored.** Every viewer gets the same raw value; the argument only feeds the log.
- **Unbound endpoints are legal.** `link` to a name nobody bound works, and `reachable` will list such names.
- **`README` performance table and "~5ms" claims are not backed by anything in the repo** (no benchmarks exist). Treat them as unverified.
- **Untracked build output.** There is no `.gitignore`; `target/` and `Cargo.lock` show up as untracked after building.

## 7. How it composes

- **As a library:** `crate-type = ["cdylib", "rlib"]`, so Rust code can depend on it as a normal crate (that is how the probe in 4b used it) and the same source produces the `.wasm`.
- **With a JS host:** through `wasm-bindgen` glue (`--target web` for browsers, `--target nodejs` for Node, as in the blueprint). The host owns the VM instance; nothing is shared between instances.
- **With persistence:** `QuiltVM` derives serde traits, so you can snapshot it as JSON on the Rust side. The wasm wrapper does not expose that; you would add a method.
- **With the wider project:** the README places this as "Layer 1" among sibling repos (quilt-types, quilt-linker, quilt-vm-typescript, and others). I did not read those repositories; the claim that they share semantics is the README's, not something I verified.

## 8. Where to look next

- [`src/lib.rs`](../src/lib.rs): the entire implementation, tests, and bindings in one file.
- [blueprint-build-and-run-in-wasm.md](blueprint-build-and-run-in-wasm.md): the build-and-run recipe for the WASM module.
- [`www/index.html`](../www/index.html): the browser demo, useful mainly as a list of the calls a page makes (and as an example of the `Map` pitfall).
