// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "tauri-plugin-cortex-ios",
    platforms: [.iOS(.v16)],
    products: [
        .library(name: "tauri-plugin-cortex-ios", type: .static, targets: ["tauri-plugin-cortex-ios"])
    ],
    dependencies: [
        // Tauri's iOS API package — the CLI copies it next to every plugin on `ios init`.
        .package(name: "Tauri", path: "../.tauri/tauri-api"),
        // The shared module (recorder + Live Activity + App Group). Also linked into the
        // widget extension, so the RecordingAttributes type identity is shared across both.
        .package(name: "CortexShared", path: "../../ios/CortexShared"),
    ],
    targets: [
        .target(
            name: "tauri-plugin-cortex-ios",
            dependencies: [
                .byName(name: "Tauri"),
                .product(name: "CortexShared", package: "CortexShared"),
            ],
            path: "Sources/CortexIosPlugin"
        )
    ]
)
