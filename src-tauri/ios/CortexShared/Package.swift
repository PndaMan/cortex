// swift-tools-version:5.9
// CortexShared — the ONE Swift module shared by the iOS app (via the Tauri plugin)
// and the CortexWidgets extension. It must be a single module so the `RecordingAttributes`
// ActivityAttributes type has ONE identity across both processes — that is what lets the
// app start a Live Activity that the widget extension actually renders.
import PackageDescription

let package = Package(
    name: "CortexShared",
    platforms: [.iOS(.v16)],
    products: [
        .library(name: "CortexShared", targets: ["CortexShared"])
    ],
    targets: [
        .target(name: "CortexShared", path: "Sources/CortexShared")
    ]
)
