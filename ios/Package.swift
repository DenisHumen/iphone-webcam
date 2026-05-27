// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "ClearCam",
    products: [
        .library(name: "ClearCamProtocol", targets: ["ClearCamProtocol"])
    ],
    targets: [
        .target(name: "ClearCamProtocol", path: "Sources/ClearCamProtocol"),
        .testTarget(
            name: "ClearCamProtocolTests",
            dependencies: ["ClearCamProtocol"],
            path: "Tests/ClearCamProtocolTests"
        ),
    ]
)
