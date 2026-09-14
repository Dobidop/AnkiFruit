// swift-tools-version: 5.9
// AnkiFruitKit — Swift wrapper over the ankifruit-core C ABI.
//
// AnkiFruitCore.xcframework is a build artifact, not a checked-in file. Build
// it first with `tools/build-xcframework.sh`, which drops it into Frameworks/.

import PackageDescription

let package = Package(
    name: "AnkiFruitKit",
    platforms: [.iOS(.v15), .macOS(.v12)],
    products: [
        .library(name: "AnkiFruitKit", targets: ["AnkiFruitKit"]),
    ],
    targets: [
        // Produced by tools/build-xcframework.sh from the Rust static library.
        .binaryTarget(
            name: "AnkiFruitCore",
            path: "Frameworks/AnkiFruitCore.xcframework"
        ),
        .target(
            name: "AnkiFruitKit",
            dependencies: ["AnkiFruitCore"],
            linkerSettings: [
                // rustls resolves native roots through Security on Apple platforms.
                .linkedFramework("Security"),
                .linkedFramework("CoreFoundation"),
            ]
        ),
        .testTarget(
            name: "AnkiFruitKitTests",
            dependencies: ["AnkiFruitKit"]
        ),
    ]
)
