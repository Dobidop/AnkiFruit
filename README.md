# AnkiFruit

An Anki client for iPhone, distributed by sideloading (AltStore / SideStore).

AnkiFruit is **not** a reimplementation of Anki. It embeds Anki's own Rust
backend — `rslib`, the same code that powers Anki Desktop and AnkiDroid — and
puts a web UI in front of it inside a Capacitor shell. Scheduling, card
rendering, `.apkg` import and AnkiWeb sync are therefore Anki's real
implementations, not lookalikes.

> **Status: Phase 0 (spike).** Not usable yet. The current goal is narrow and
> deliberate: prove that `rslib` cross-compiles to Apple mobile targets before
> any app code is written. See [Project status](#project-status).

## Why this architecture

Anki Desktop already *is* a web UI on a Rust backend. Its Svelte frontend talks
to `rslib` by POSTing protobuf to `/_anki/<method>` on a loopback HTTP server
(`ts/lib/generated/post.ts` → `qt/aqt/mediasrv.py`). AnkiDroid uses the same
`rslib` core behind a JNI bridge.

AnkiFruit reuses that contract verbatim:

```
┌──────────────────────────────────────────┐
│ Capacitor WKWebView                      │
│   React UI                               │
│   Anki's generated TS client (postProto) │
└────────────────┬─────────────────────────┘
                 │  POST /_anki/<method>
                 │  Content-Type: application/binary
                 │  Authorization: Bearer <token>
                 ▼
┌──────────────────────────────────────────┐
│ 127.0.0.1:<ephemeral>  (axum, in-process)│
│   routes.rs  method → (service, method)  │
└────────────────┬─────────────────────────┘
                 │  Backend::run_service_method(svc, mth, &[u8])
                 ▼
┌──────────────────────────────────────────┐
│ Anki rslib (vendor/anki, pinned)         │
│   scheduler · sync · import/export · …   │
└──────────────────────────────────────────┘
```

Because the wire contract matches upstream, Anki's **own generated TypeScript
client** works against this server essentially unchanged — including the
protobuf message types. That is the main payoff of the design: the UI layer
talks to a real Anki backend without us hand-writing a scheduler, a template
renderer, or a sync client.

A loopback port on iOS is reachable by any app on the device, so every request
must carry a bearer token minted at startup. See `rust/ankifruit-core/src/server.rs`.

## Repository layout

| Path | Contents |
|---|---|
| `rust/ankifruit-core/` | The bridge: C ABI for Swift + the loopback protobuf server |
| `rust/ankifruit-core/build.rs` | Generates the method routing table from Anki's descriptor pool |
| `rust/ankifruit-core/include/` | `ankifruit.h` + modulemap consumed by Swift |
| `vendor/anki/` | Anki upstream, pinned submodule (currently 26.08.1, `4383a707`) |
| `web/` | React UI (not started) |
| `ios/` | Capacitor/Xcode shell (not started) |
| `.github/workflows/ios-spike.yml` | Phase 0: cross-compile `rslib` to iOS |

## Method routing

Anki dispatches every backend call through a single entry point:

```rust
Backend::run_service_method(service: u32, method: u32, input: &[u8])
    -> Result<Vec<u8>, Vec<u8>>
```

The two indices are assigned by `anki_proto_gen::get_services()`. Upstream is
explicit that clients must use the `.index` fields it returns rather than
recomputing them by enumeration, so `build.rs` reads Anki's protobuf descriptor
pool at build time and generates:

- `lookup(method)` — for Anki-compatible `/_anki/<method>` paths, emitted only
  for method names that are unique across services.
- `lookup_qualified(service, method)` — for `/_anki/<Service>/<method>`, always
  unambiguous.

Regenerating against a newer Anki is therefore just a submodule bump. Note that
Anki's `.proto` files are an internal API with no stability guarantee; treat an
upstream bump as a migration, not a routine update.

## Building

### Requirements

- Rust **1.97.1** (pinned in `rust/rust-toolchain.toml` to match upstream Anki;
  older toolchains fail to resolve rslib's dependency graph)
- `protoc` on `PATH`, or `PROTOC` pointing at the binary
- Submodules checked out **recursively** — `vendor/anki` has its own
  `ftl/core-repo` and `ftl/qt-repo` submodules holding the translations.
  Without them, rslib's i18n build script panics in `gather.rs`.

```bash
git clone --recurse-submodules <this repo>
cd ankifruit/rust
cargo build --release
```

### Running the app without a Mac

The backend serves the web UI, so the whole app runs in a desktop browser:

```bash
cd web && npm install && npm run build
cd ../rust && cargo run --bin dev
```

That prints a loopback URL serving the real Anki backend. UI work needs no
Apple hardware; only the `.ipa` does.

### iOS

An `.ipa` can only be produced on macOS. `.github/workflows/ios-spike.yml`
cross-compiles the static library on a macOS runner and assembles
`AnkiFruitCore.xcframework` with device and simulator slices.

## Project status

Phase 0 existed to retire the one risk that could invalidate the whole
approach: nobody upstream maintains an Apple-mobile build of `rslib`.

**The spike passed.** `rslib` cross-compiles cleanly to both
`aarch64-apple-ios` and `aarch64-apple-ios-sim` with plain cargo, and the two
slices assemble into a usable `AnkiFruitCore.xcframework`. No patches to
upstream Anki were needed. The cold build takes about 4 minutes on a
`macos-15` runner.

| | Status |
|---|---|
| rslib builds standalone with plain cargo (no Ninja orchestration) | ✅ verified on Windows |
| `ankifruit-core` builds against rslib | ✅ verified on Windows |
| Routing table generates from Anki's descriptor pool | ✅ 236 methods, all unambiguous |
| Loopback server round-trips a real call into rslib | ✅ `tests/roundtrip.rs` |
| Bearer token rejects unauthorized callers | ✅ `tests/roundtrip.rs` |
| rslib cross-compiles to `aarch64-apple-ios` | ✅ CI, both device + sim |
| `AnkiFruitCore.xcframework` assembles | ✅ CI artifact, 84 MB |
| Swift calls the backend on a real simulator | ✅ 5/5 XCTest on iPhone 16 Pro |
| Web UI drives the backend end to end | ✅ verified in a browser |
| Unsigned `.ipa` builds for sideloading | ✅ 6.7 MB |
| `.apkg` import · review UI · AnkiWeb sync | not started |

Phase 1a: `AnkiFruitKit` opens a real collection on an iOS simulator and reads
its decks back over the loopback server. App Transport Security does not
interfere with loopback HTTP, and unauthenticated callers are rejected.

Phase 1b: the React UI encodes protobuf, opens a real collection through rslib
and lists its decks. A `WKWebView` shell packages it into a sideloadable
`.ipa`.

### Install size

The device static archive is 158 MB, but that is object files and metadata;
the linker keeps only what is reachable.

| | size |
|---|---|
| `.ipa` (what AltStore re-signs) | 6.7 MB |
| installed bundle | 18 MB |
| binary | 17.7 MB |

Small enough that AltStore's weekly re-sign cycle is not a burden.

`tests/roundtrip.rs` opens a real collection on disk and lists its decks
through the HTTP surface, so everything except the Apple target triple is
confirmed working locally.

The pure-TypeScript fallback (`ts-fsrs`, hand-written template renderer and
sync client) is no longer needed and has been dropped.

That open question about install size is now answered above: 6.7 MB.

## Licensing

`rslib` is **AGPL-3.0-or-later**, so AnkiFruit is too, and its source must be
published. AGPL is incompatible with the App Store's terms — this is a
sideload-only app by construction.

Anki is a trademark of Ankitects Pty Ltd. AnkiFruit is an independent project
and is not affiliated with or endorsed by Ankitects.
