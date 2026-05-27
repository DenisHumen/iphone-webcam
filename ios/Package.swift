// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "ClearCam",
    platforms: [.iOS(.v17), .macOS(.v13)],
    products: [
        .library(name: "ClearCamProtocol", targets: ["ClearCamProtocol"]),
        .library(name: "ClearCamCore", targets: ["ClearCamCore"]),
    ],
    targets: [
        .target(name: "ClearCamProtocol", path: "Sources/ClearCamProtocol"),
        .target(
            name: "ClearCamCore",
            dependencies: ["ClearCamProtocol"],
            path: "Sources/ClearCamCore"
        ),
        .testTarget(
            name: "ClearCamProtocolTests",
            dependencies: ["ClearCamProtocol"],
            path: "Tests/ClearCamProtocolTests"
        ),
        .testTarget(
            name: "ClearCamCoreTests",
            dependencies: ["ClearCamCore"],
            path: "Tests/ClearCamCoreTests"
        ),
    ]
)
