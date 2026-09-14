// AnkiFruit — generates the HTTP method routing table from Anki's protobuf
// descriptor pool.
//
// Anki dispatches every backend call through
// `Backend::run_service_method(service: u32, method: u32, input: &[u8])`.
// The two indices are assigned by `anki_proto_gen::get_services()`. Upstream is
// explicit that clients must use the `.index` fields it returns rather than
// recomputing them by enumeration, so that is what we do here.
//
// We build the descriptor pool ourselves rather than reading the one
// `anki_proto`'s build script leaves in the target directory: Cargo only orders
// build scripts against *build*-dependencies, and `anki_proto` is a regular
// dependency, so that file is not guaranteed to exist when we run. It happened
// to work for host builds and broke when cross-compiling to iOS.
//
// Ordering must match upstream or every index shifts. `rslib/proto/rust.rs`
// sorts the proto paths and hands them to prost-build, which invokes protoc
// with --include_imports; we do exactly the same. The round-trip tests are the
// real guard: a wrong mapping would dispatch to the wrong method and fail.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use anki_proto_gen::get_services;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use prost_reflect::DescriptorPool;

/// `Method::from_proto` stores names already lowered to snake_case; Anki's
/// TypeScript client camel-cases them to build the `/_anki/<method>` path.
fn to_camel_case(snake: &str) -> String {
    let mut out = String::with_capacity(snake.len());
    let mut capitalize = false;
    for ch in snake.chars() {
        if ch == '_' {
            capitalize = true;
        } else if capitalize {
            out.extend(ch.to_uppercase());
            capitalize = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn protoc_binary() -> String {
    env::var("PROTOC_BINARY")
        .or_else(|_| env::var("PROTOC"))
        .unwrap_or_else(|_| "protoc".to_string())
}

/// Compiles Anki's protos into a FileDescriptorSet, mirroring prost-build.
fn build_descriptor_pool(out_dir: &PathBuf) -> Result<DescriptorPool> {
    let proto_dir = PathBuf::from("../../vendor/anki/proto");
    let anki_dir = proto_dir.join("anki");
    if !anki_dir.is_dir() {
        bail!(
            "{} not found - is the vendor/anki submodule checked out?",
            anki_dir.display()
        );
    }

    // Sorted, exactly as rslib/proto/rust.rs gathers them.
    let mut paths: Vec<PathBuf> = fs::read_dir(&anki_dir)?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "proto"))
        .collect();
    paths.sort();
    if paths.is_empty() {
        bail!("no .proto files under {}", anki_dir.display());
    }
    for path in &paths {
        println!("cargo:rerun-if-changed={}", path.display());
    }

    let descriptors = out_dir.join("anki_descriptors.bin");
    let protoc = protoc_binary();
    let output = Command::new(&protoc)
        .arg("--include_imports")
        // anki_proto_gen reads doc comments out of source_code_info and
        // unwraps it; protoc omits that section unless asked for it.
        .arg("--include_source_info")
        .arg(format!("--descriptor_set_out={}", descriptors.display()))
        .arg("-I")
        .arg(&proto_dir)
        .args(&paths)
        .output()
        .with_context(|| format!("failed to run protoc ({protoc}); is it installed?"))?;
    if !output.status.success() {
        bail!(
            "protoc failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    DescriptorPool::decode(fs::read(&descriptors)?.as_ref()).context("decode descriptor pool")
}

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=build.rs");
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let pool = build_descriptor_pool(&out_dir)?;
    let (_col_services, backend_services) = get_services(&pool);

    // (http_name, service_index, method_index, service_name)
    let mut routes: Vec<(String, usize, usize, String)> = Vec::new();
    for service in &backend_services {
        // The frontend service is not dispatched through run_service_method.
        if service.name == "BackendFrontendService" {
            continue;
        }
        for method in service.all_methods() {
            routes.push((
                to_camel_case(&method.name),
                service.index,
                method.index,
                service.name.clone(),
            ));
        }
    }
    if routes.is_empty() {
        bail!("no backend services found in the descriptor pool");
    }

    // A bare `/_anki/<method>` path is only unambiguous when the method name is
    // unique across services. Ambiguous names stay reachable via the qualified
    // `/_anki/<Service>/<method>` form.
    let flat: Vec<&(String, usize, usize, String)> = routes
        .iter()
        .filter(|route| routes.iter().filter(|r| r.0 == route.0).count() == 1)
        .collect();

    let mut out = String::new();
    out.push_str("// @generated by build.rs - do not edit.\n\n");
    out.push_str("/// Resolves an unqualified `/_anki/<method>` path.\n");
    out.push_str("pub fn lookup(method: &str) -> Option<(u32, u32)> {\n    Some(match method {\n");
    for (name, svc, mth, _) in &flat {
        out.push_str(&format!("        {name:?} => ({svc}, {mth}),\n"));
    }
    out.push_str("        _ => return None,\n    })\n}\n\n");

    out.push_str("/// Resolves a qualified `/_anki/<Service>/<method>` path.\n");
    out.push_str(
        "pub fn lookup_qualified(service: &str, method: &str) -> Option<(u32, u32)> {\n    Some(match (service, method) {\n",
    );
    for (name, svc, mth, svc_name) in &routes {
        let short = svc_name.trim_start_matches("Backend");
        out.push_str(&format!("        ({short:?}, {name:?}) => ({svc}, {mth}),\n"));
    }
    out.push_str("        _ => return None,\n    })\n}\n\n");

    out.push_str(&format!(
        "/// Every routable method, as `(service_index, method_index, service, method)`.\npub const ROUTES: [(u32, u32, &str, &str); {}] = [\n",
        routes.len()
    ));
    for (name, svc, mth, svc_name) in &routes {
        let short = svc_name.trim_start_matches("Backend");
        out.push_str(&format!("    ({svc}, {mth}, {short:?}, {name:?}),\n"));
    }
    out.push_str("];\n");

    fs::write(out_dir.join("routes.rs"), out)?;
    println!(
        "cargo:warning=ankifruit: generated {} routes ({} unqualified)",
        routes.len(),
        flat.len()
    );
    Ok(())
}
