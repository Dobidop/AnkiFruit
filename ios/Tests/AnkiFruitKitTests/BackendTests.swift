// Proves the Swift -> C -> Rust -> rslib -> SQLite path works on a real
// iOS simulator. These are the tests that retire the FFI risk; everything
// below the C ABI is already covered by rust/ankifruit-core/tests/roundtrip.rs.
//
// Protobuf messages are hand-encoded here on purpose: the generated Swift
// message types arrive in phase 1b, and this file only needs a couple of
// fields to demonstrate a real call reaching the backend.

import XCTest
@testable import AnkiFruitKit

/// Minimal protobuf wire-format writer — enough for string and bool fields.
private struct ProtoWriter {
    private(set) var bytes = Data()

    mutating func string(field: Int, _ value: String) {
        let utf8 = Data(value.utf8)
        varint(UInt64(field << 3 | 2))   // wire type 2: length-delimited
        varint(UInt64(utf8.count))
        bytes.append(utf8)
    }

    mutating func bool(field: Int, _ value: Bool) {
        guard value else { return }      // proto3 omits default values
        varint(UInt64(field << 3 | 0))   // wire type 0: varint
        varint(1)
    }

    private mutating func varint(_ value: UInt64) {
        var v = value
        repeat {
            var byte = UInt8(v & 0x7F)
            v >>= 7
            if v != 0 { byte |= 0x80 }
            bytes.append(byte)
        } while v != 0
    }
}

final class BackendTests: XCTestCase {
    private func makeBackend() throws -> AnkiFruitBackend {
        try AnkiFruitBackend(preferredLanguages: ["en"], port: 0)
    }

    func testBackendStartsAndReportsPortAndToken() throws {
        let backend = try makeBackend()
        XCTAssertGreaterThan(backend.port, 0, "backend should bind an ephemeral port")
        XCTAssertEqual(backend.token.count, 32, "expected a 32-character token")
    }

    func testLinkedAnkiBuildHashIsExposed() throws {
        let hash = AnkiFruitBackend.ankiBuildHash
        XCTAssertFalse(hash.isEmpty, "rslib build hash should be readable through the FFI")
    }

    func testHealthEndpointResponds() async throws {
        let backend = try makeBackend()
        let body = try await backend.health()
        XCTAssertFalse(body.isEmpty, "healthz should return the rslib build hash")
    }

    func testRequestsWithoutTokenAreRejected() async throws {
        let backend = try makeBackend()
        var req = URLRequest(url: backend.baseURL.appendingPathComponent("_anki/getDeckNames"))
        req.httpMethod = "POST"
        req.setValue("application/binary", forHTTPHeaderField: "Content-Type")
        // deliberately no Authorization header

        let (_, response) = try await URLSession.shared.data(for: req)
        XCTAssertEqual(
            (response as? HTTPURLResponse)?.statusCode, 401,
            "the loopback port must not be usable by other apps on the device"
        )
    }

    /// The real thing: open a collection on disk and read its decks back.
    func testOpensCollectionAndListsDecks() async throws {
        let backend = try makeBackend()
        let dir = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }

        let collection = dir.appendingPathComponent("collection.anki2")

        var open = ProtoWriter()
        open.string(field: 1, collection.path)
        open.string(field: 2, dir.appendingPathComponent("media").path)
        open.string(field: 3, dir.appendingPathComponent("media.db").path)
        _ = try await backend.call(method: "openCollection", body: open.bytes)

        XCTAssertTrue(
            FileManager.default.fileExists(atPath: collection.path),
            "rslib should have created the collection file"
        )

        var decks = ProtoWriter()
        decks.bool(field: 2, true)   // include_filtered
        let response = try await backend.call(method: "getDeckNames", body: decks.bytes)

        // DeckNames carries DeckNameId { id, name }; the default deck is enough
        // to prove the call reached rslib and came back decoded.
        XCTAssertTrue(
            response.range(of: Data("Default".utf8)) != nil,
            "expected the Default deck in the response, got \(response.count) bytes"
        )

        _ = try await backend.call(method: "closeCollection", body: Data())
    }
}
