// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "notchtap-detect",
    platforms: [.macOS(.v13)],
    targets: [
        .executableTarget(name: "notchtap-detect", path: "Sources/notchtap-detect")
    ]
)
