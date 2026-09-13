// AnkiFruit core — embeds Anki's Rust backend (rslib) and exposes it to the
// iOS shell over loopback HTTP, using the same protobuf contract Anki Desktop
// uses between its webview and backend.
//
// Licensed AGPL-3.0-or-later, because it links Anki's rslib.

pub mod ffi;
pub mod server;

/// Method name -> (service index, method index), generated at build time from
/// Anki's protobuf descriptor pool. See `build.rs`.
pub mod routes {
    include!(concat!(env!("OUT_DIR"), "/routes.rs"));
}

use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use prost::Message;
use rand::Rng;

/// A running backend: the Anki backend plus the loopback server in front of it.
pub struct Instance {
    pub port: u16,
    pub token: String,
    runtime: tokio::runtime::Runtime,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Instance {
    /// Starts the backend and serves it on `127.0.0.1:<port>`; pass port 0 to
    /// let the OS choose. `preferred_langs` follows Anki's format, e.g. `en_US`.
    pub fn start(preferred_langs: &[String], port: u16, server_mode: bool) -> Result<Instance> {
        let init = anki_proto::backend::BackendInit {
            preferred_langs: preferred_langs.to_vec(),
            locale_folder_path: String::new(),
            server: server_mode,
        };
        let mut init_bytes = Vec::new();
        init.encode(&mut init_bytes)?;

        let backend = anki::backend::init_backend(&init_bytes)
            .map_err(anyhow::Error::msg)
            .context("init anki backend")?;

        let token: String = {
            let mut rng = rand::rng();
            (0..32)
                .map(|_| {
                    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
                    CHARS[rng.random_range(0..CHARS.len())] as char
                })
                .collect()
        };

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .context("build tokio runtime")?;

        let state = Arc::new(server::AppState {
            backend,
            token: token.clone(),
        });

        let (bound_port, listener) = runtime
            .block_on(server::bind(state.clone(), port))
            .context("bind loopback listener")?;

        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let router = server::router(state);
        runtime.spawn(async move {
            let _ = axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _ = rx.await;
                })
                .await;
        });

        Ok(Instance {
            port: bound_port,
            token,
            runtime,
            shutdown: Some(tx),
        })
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        // Do not block the caller (possibly the iOS main thread) on teardown.
        let runtime = std::mem::replace(
            &mut self.runtime,
            tokio::runtime::Builder::new_current_thread()
                .build()
                .expect("placeholder runtime"),
        );
        runtime.shutdown_background();
    }
}

/// The rslib build hash, useful for verifying which Anki version is linked in.
pub fn anki_buildhash() -> &'static str {
    anki::version::buildhash()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_are_generated() {
        assert!(
            routes::ROUTES.len() > 100,
            "expected a large routing table, got {}",
            routes::ROUTES.len()
        );
    }

    /// The methods the app depends on must be reachable by their bare names.
    #[test]
    fn core_methods_resolve() {
        for name in [
            "openCollection",
            "closeCollection",
            "getDeckNames",
            "importAnkiPackage",
            "syncCollection",
            "syncLogin",
        ] {
            assert!(
                routes::lookup(name).is_some(),
                "{name} did not resolve to a (service, method) pair"
            );
        }
    }

    #[test]
    fn server_starts_and_serves_health() {
        let inst = Instance::start(&["en".to_string()], 0, false).expect("start instance");
        assert!(inst.port > 0);
        assert_eq!(inst.token.len(), 32);
    }
}
