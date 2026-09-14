// End-to-end proof of the bridge: HTTP request -> route lookup -> protobuf ->
// rslib -> SQLite, and back. If this passes, the Capacitor webview can drive a
// real Anki backend using Anki's own generated TypeScript client.

use ankifruit_core::routes;
use ankifruit_core::Instance;
use anki_proto::collection::OpenCollectionRequest;
use anki_proto::decks::DeckNames;
use anki_proto::decks::GetDeckNamesRequest;
use prost::Message;

struct Client {
    inst: Instance,
    http: reqwest::blocking::Client,
}

impl Client {
    fn start() -> Client {
        let inst = Instance::start(&["en".to_string()], 0, false, None).expect("start backend");
        Client {
            inst,
            http: reqwest::blocking::Client::new(),
        }
    }

    /// Mirrors Anki's `postProto`: protobuf in, protobuf out.
    fn call<Req: Message, Resp: Message + Default>(&self, method: &str, req: Req) -> Resp {
        let resp = self.raw(method, req.encode_to_vec(), &self.inst.token);
        let status = resp.status();
        let body = resp.bytes().expect("read body");
        assert!(
            status.is_success(),
            "{method} failed with {status}: {}",
            String::from_utf8_lossy(&body)
        );
        Resp::decode(body).expect("decode response")
    }

    fn raw(&self, method: &str, body: Vec<u8>, token: &str) -> reqwest::blocking::Response {
        self.http
            .post(format!("http://127.0.0.1:{}/_anki/{method}", self.inst.port))
            .header("Content-Type", "application/binary")
            .header("Authorization", format!("Bearer {token}"))
            .body(body)
            .send()
            .expect("send request")
    }
}

#[test]
fn opens_a_collection_and_lists_decks() {
    let dir = tempfile::tempdir().expect("tempdir");
    let col = dir.path().join("collection.anki2");
    let client = Client::start();

    // Creates the collection file if it does not exist.
    let _: anki_proto::generic::Empty = client.call(
        "openCollection",
        OpenCollectionRequest {
            collection_path: col.to_str().unwrap().to_string(),
            media_folder_path: dir.path().join("media").to_str().unwrap().to_string(),
            media_db_path: dir.path().join("media.db").to_str().unwrap().to_string(),
        },
    );
    assert!(col.exists(), "rslib did not create the collection file");

    // A fresh collection always has the Default deck.
    let decks: DeckNames = client.call(
        "getDeckNames",
        GetDeckNamesRequest {
            skip_empty_default: false,
            include_filtered: true,
        },
    );
    let names: Vec<&str> = decks.entries.iter().map(|e| e.name.as_str()).collect();
    assert!(
        names.iter().any(|n| *n == "Default"),
        "expected a Default deck, got {names:?}"
    );

    let _: anki_proto::generic::Empty = client.call(
        "closeCollection",
        anki_proto::collection::CloseCollectionRequest {
            downgrade_to_schema11: false,
        },
    );
}

#[test]
fn rejects_requests_without_the_token() {
    let client = Client::start();
    let resp = client.raw("getDeckNames", vec![], "wrong-token");
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::UNAUTHORIZED,
        "loopback port must not be usable by other apps on the device"
    );
}

#[test]
fn unknown_methods_are_not_found() {
    let client = Client::start();
    let resp = client.raw("noSuchMethod", vec![], &client.inst.token);
    assert_eq!(resp.status(), reqwest::StatusCode::NOT_FOUND);
}

#[test]
fn the_methods_v1_needs_are_routable() {
    // Deliberately spelled as the UI will call them.
    for name in [
        "openCollection",
        "closeCollection",
        "getDeckNames",
        "importAnkiPackage",
        "syncCollection",
        "syncLogin",
        "getQueuedCards",
        "answerCard",
        "renderExistingCard",
    ] {
        assert!(routes::lookup(name).is_some(), "{name} is not routable");
    }
}

/// rslib reads its build hash from vendor/anki/out/buildhash, which only
/// Anki's Ninja build writes. We build with plain cargo, so an empty hash here
/// means tools/write-buildhash.sh was not run and we cannot tell which Anki is
/// linked in.
#[test]
fn health_reports_which_anki_is_linked() {
    let client = Client::start();
    let body = reqwest::blocking::Client::new()
        .get(format!("http://127.0.0.1:{}/_anki/healthz", client.inst.port))
        .header("Authorization", format!("Bearer {}", client.inst.token))
        .send()
        .expect("healthz")
        .text()
        .expect("body");

    assert!(body.contains(r#""ok":true"#), "unexpected body: {body}");
    assert!(
        !body.contains(r#""anki":"""#),
        "anki build hash is empty - run tools/write-buildhash.sh before cargo build: {body}"
    );
}

/// The web UI is served from the backend's own origin so the webview needs no
/// CORS handling. Assets stay unauthenticated on purpose — they are inert and
/// ship inside the app — which is exactly why the token must never be baked
/// into them.
#[test]
fn serves_the_web_ui_from_the_same_origin() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("index.html"), "<!doctype html><title>AnkiFruit</title>")
        .expect("write index");
    std::fs::write(dir.path().join("app.js"), "export const x = 1;").expect("write asset");

    let inst = Instance::start(
        &["en".to_string()],
        0,
        false,
        Some(dir.path().to_path_buf()),
    )
    .expect("start backend");
    let http = reqwest::blocking::Client::new();
    let base = format!("http://127.0.0.1:{}", inst.port);

    let index = http.get(&base).send().expect("GET /");
    assert!(index.status().is_success(), "index status {}", index.status());
    assert!(index.text().expect("body").contains("AnkiFruit"));

    let asset = http.get(format!("{base}/app.js")).send().expect("GET asset");
    assert!(asset.status().is_success());

    // Unknown paths fall back to index.html so client-side routing works.
    let deep = http.get(format!("{base}/decks/42")).send().expect("GET route");
    assert!(deep.status().is_success(), "SPA fallback status {}", deep.status());

    // Serving assets must not have opened up the API.
    let api = http
        .post(format!("{base}/_anki/getDeckNames"))
        .send()
        .expect("POST api");
    assert_eq!(
        api.status(),
        reqwest::StatusCode::UNAUTHORIZED,
        "static serving must not bypass API auth"
    );
}
