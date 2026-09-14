// Hosts the web UI and owns the backend that serves it.
//
// The backend serves both the API and the built web assets, so the webview is
// same-origin with the API and no CORS handling is required. The bearer token
// is handed to the page through a user script rather than embedded in the
// served HTML: the loopback port is reachable by any app on the device, and
// anything written into the HTML would be readable by them too.

import UIKit
import WebKit
import AnkiFruitKit

final class ReviewerViewController: UIViewController {
    private var backend: AnkiFruitBackend?
    private var webView: WKWebView?

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground

        do {
            let backend = try AnkiFruitBackend(
                preferredLanguages: Self.preferredLanguages(),
                webRoot: Self.bundledWebRoot()
            )
            self.backend = backend
            installWebView(for: backend)
        } catch {
            showFailure("\(error)")
        }
    }

    /// The web build is copied into the bundle as a folder reference.
    private static func bundledWebRoot() -> URL? {
        Bundle.main.resourceURL?.appendingPathComponent("web")
    }

    private static func preferredLanguages() -> [String] {
        // Anki expects e.g. "en_US"; Foundation gives us "en-US".
        Locale.preferredLanguages.prefix(3).map { $0.replacingOccurrences(of: "-", with: "_") }
    }

    /// Collections live in Application Support, which is backed up but kept out
    /// of the user's Files view — it is a database, not a document.
    private static func dataDirectory() throws -> URL {
        let base = try FileManager.default.url(
            for: .applicationSupportDirectory,
            in: .userDomainMask,
            appropriateFor: nil,
            create: true
        )
        let dir = base.appendingPathComponent("AnkiFruit", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }

    private func installWebView(for backend: AnkiFruitBackend) {
        let dataDir: URL
        do {
            dataDir = try Self.dataDirectory()
        } catch {
            showFailure("could not create data directory: \(error)")
            return
        }

        let config = WKWebViewConfiguration()
        let bootstrap = """
        window.__ANKIFRUIT__ = {
            token: \(jsString(backend.token)),
            baseUrl: \(jsString(backend.baseURL.absoluteString)),
            dataDir: \(jsString(dataDir.path))
        };
        """
        config.userContentController.addUserScript(
            WKUserScript(source: bootstrap, injectionTime: .atDocumentStart, forMainFrameOnly: true)
        )

        let webView = WKWebView(frame: view.bounds, configuration: config)
        webView.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        // The page manages its own safe-area padding.
        webView.scrollView.contentInsetAdjustmentBehavior = .never
        view.addSubview(webView)
        self.webView = webView

        webView.load(URLRequest(url: backend.baseURL))
    }

    /// JSON-encodes a string so it is safe to paste into injected JavaScript.
    private func jsString(_ value: String) -> String {
        let data = try? JSONSerialization.data(withJSONObject: [value], options: [])
        guard let data, let array = String(data: data, encoding: .utf8) else { return "\"\"" }
        return String(array.dropFirst().dropLast())
    }

    private func showFailure(_ message: String) {
        let label = UILabel(frame: view.bounds.insetBy(dx: 24, dy: 24))
        label.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        label.numberOfLines = 0
        label.textAlignment = .center
        label.text = "AnkiFruit could not start.\n\n\(message)"
        view.addSubview(label)
    }
}
