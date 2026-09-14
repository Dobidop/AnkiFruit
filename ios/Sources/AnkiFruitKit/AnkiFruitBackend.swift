// AnkiFruitKit — Swift façade over the ankifruit-core C ABI.
//
// The backend runs Anki's rslib in-process and exposes it over a loopback HTTP
// server speaking Anki's protobuf contract (POST /_anki/<method>). This type
// owns that server's lifetime: it starts on init and shuts down on deinit.
//
// The loopback port is reachable by any app on the device, so every request
// must carry `authorizationHeader`. Never hand the port out without the token.

import Foundation
import AnkiFruitCore

public enum AnkiFruitError: Error, CustomStringConvertible {
    case startFailed(String)
    case badResponse(status: Int, body: String)

    public var description: String {
        switch self {
        case .startFailed(let msg):
            return "failed to start Anki backend: \(msg)"
        case .badResponse(let status, let body):
            return "backend returned \(status): \(body)"
        }
    }
}

public final class AnkiFruitBackend {
    private let handle: OpaquePointer

    /// Loopback port the backend is listening on.
    public let port: UInt16

    /// Bearer token required on every request to the backend.
    public let token: String

    /// Build hash of the linked rslib, for verifying which Anki version is in.
    public static var ankiBuildHash: String {
        guard let raw = ankifruit_anki_buildhash() else { return "unknown" }
        defer { ankifruit_string_free(raw) }
        return String(cString: raw)
    }

    /// Starts the backend. Pass `port: 0` to let the OS choose one.
    ///
    /// `webRoot` is the directory holding the built web UI. Serving it from the
    /// backend's own origin keeps the webview same-origin, so no CORS setup is
    /// needed. Pass nil to run the API alone.
    public init(
        preferredLanguages: [String] = ["en"],
        webRoot: URL? = nil,
        port: UInt16 = 0
    ) throws {
        var errPtr: UnsafeMutablePointer<CChar>?
        let langs = preferredLanguages.joined(separator: ",")

        let started: OpaquePointer? = langs.withCString { langsPtr in
            if let webRoot {
                return webRoot.path.withCString { rootPtr in
                    ankifruit_start(langsPtr, rootPtr, port, &errPtr)
                }
            }
            return ankifruit_start(langsPtr, nil, port, &errPtr)
        }

        guard let handle = started else {
            let message: String
            if let errPtr {
                message = String(cString: errPtr)
                ankifruit_string_free(errPtr)
            } else {
                message = "unknown error"
            }
            throw AnkiFruitError.startFailed(message)
        }

        self.handle = handle
        self.port = ankifruit_port(handle)

        if let rawToken = ankifruit_token(handle) {
            self.token = String(cString: rawToken)
            ankifruit_string_free(rawToken)
        } else {
            self.token = ""
        }
    }

    deinit {
        ankifruit_stop(handle)
    }

    /// Root the webview and HTTP calls should target.
    public var baseURL: URL {
        URL(string: "http://127.0.0.1:\(port)")!
    }

    public var authorizationHeader: String {
        "Bearer \(token)"
    }

    /// Builds a request for one backend method, matching Anki's `postProto`.
    public func request(method: String, body: Data) -> URLRequest {
        var req = URLRequest(url: baseURL.appendingPathComponent("_anki/\(method)"))
        req.httpMethod = "POST"
        req.setValue("application/binary", forHTTPHeaderField: "Content-Type")
        req.setValue(authorizationHeader, forHTTPHeaderField: "Authorization")
        req.httpBody = body
        return req
    }

    /// Calls one backend method. `body` and the result are encoded protobuf.
    @discardableResult
    public func call(method: String, body: Data = Data()) async throws -> Data {
        let (data, response) = try await URLSession.shared.data(for: request(method: method, body: body))
        guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) else {
            let status = (response as? HTTPURLResponse)?.statusCode ?? -1
            throw AnkiFruitError.badResponse(
                status: status,
                body: String(data: data, encoding: .utf8) ?? "<\(data.count) bytes>"
            )
        }
        return data
    }

    /// Liveness check; returns the linked rslib build hash.
    public func health() async throws -> String {
        var req = URLRequest(url: baseURL.appendingPathComponent("_anki/healthz"))
        req.httpMethod = "GET"
        req.setValue(authorizationHeader, forHTTPHeaderField: "Authorization")
        let (data, response) = try await URLSession.shared.data(for: req)
        guard let http = response as? HTTPURLResponse, http.statusCode == 200 else {
            let status = (response as? HTTPURLResponse)?.statusCode ?? -1
            throw AnkiFruitError.badResponse(
                status: status,
                body: String(data: data, encoding: .utf8) ?? ""
            )
        }
        return String(data: data, encoding: .utf8) ?? ""
    }
}
