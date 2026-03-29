// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "apple-intelligence",
    platforms: [.macOS(.v14)],
    products: [
        .library(
            name: "AppleIntelligence",
            type: .static,
            targets: ["AppleIntelligence"]
        ),
    ],
    dependencies: [
        .package(url: "https://github.com/Brendonovich/swift-rs", from: "1.0.7"),
    ],
    targets: [
        .target(
            name: "AppleIntelligence",
            dependencies: [
                .product(name: "SwiftRs", package: "swift-rs"),
            ],
            path: "Sources/AppleIntelligence"
        ),
    ]
)
