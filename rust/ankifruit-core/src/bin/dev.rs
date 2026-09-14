// Desktop dev harness: runs the real Anki backend with the built web UI in
// front of it, so the whole app can be driven from a browser on any platform.
// The iOS shell is not involved, which is the point — UI work does not need a
// Mac.
//
// On device the bearer token is injected into the webview by the native shell
// and never touches disk. Here there is no shell, so we serve a patched copy of
// the built UI from a scratch directory. The copy is why this is a dev-only
// binary: it puts the token in a file, which is fine on a developer's machine
// and would not be on a phone.

use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use ankifruit_core::Instance;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;

fn arg(name: &str) -> Option<String> {
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if let Some(rest) = arg.strip_prefix(&format!("--{name}=")) {
            return Some(rest.to_string());
        }
        if arg == format!("--{name}") {
            return args.next();
        }
    }
    None
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &dest)?;
        } else {
            fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .context("locate repo root")?;

    let web_root = arg("web-root")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root.join("web/dist"));
    if !web_root.join("index.html").is_file() {
        bail!(
            "no built web UI at {} - run `npm run build` in web/ first",
            web_root.display()
        );
    }

    let data_dir = arg("data-dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root.join("devdata"));
    fs::create_dir_all(&data_dir).context("create data dir")?;
    // canonicalize() yields a \\?\ UNC path on Windows; strip it so the path
    // injected into the page is the ordinary one.
    let data_dir = {
        let canonical = data_dir.canonicalize()?;
        let text = canonical.to_string_lossy().to_string();
        PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(&text).to_string())
    };

    let port: u16 = arg("port").map_or(Ok(0), |p| p.parse()).context("parse --port")?;

    // Serve a patched copy so the original build output stays untouched.
    let serve_dir = env::temp_dir().join("ankifruit-dev-ui");
    let _ = fs::remove_dir_all(&serve_dir);
    copy_dir(&web_root, &serve_dir).context("stage web UI")?;

    let instance = Instance::start(&["en".to_string()], port, false, Some(serve_dir.clone()))
        .context("start backend")?;

    // ServeDir reads from disk per request, so patching after start is fine —
    // and it is the only order in which the token exists yet.
    let index_path = serve_dir.join("index.html");
    let index = fs::read_to_string(&index_path)?;
    let injected = format!(
        "<script>window.__ANKIFRUIT__={{token:{:?},baseUrl:\"\",dataDir:{:?}}};</script>",
        instance.token,
        data_dir.to_string_lossy(),
    );
    let patched = match index.split_once("</head>") {
        Some((head, tail)) => format!("{head}{injected}</head>{tail}"),
        None => format!("{injected}{index}"),
    };
    fs::write(&index_path, patched).context("inject dev config")?;

    println!("AnkiFruit dev server");
    println!("  url:       http://127.0.0.1:{}", instance.port);
    println!("  data dir:  {}", data_dir.display());
    println!("  anki:      {}", ankifruit_core::anki_buildhash());
    println!("\nPress Ctrl+C to stop.");

    // Park the main thread; the server runs on the instance's own runtime.
    loop {
        std::thread::park();
    }
}
