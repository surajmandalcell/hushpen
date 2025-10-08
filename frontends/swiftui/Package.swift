// swift-tools-version:5.7
// The swift-tools-version declares the minimum version of Swift required to build this package.

import PackageDescription

let package = Package(
    name: "ASRPro",
    platforms: [
        .macOS(.v12)
    ],
    products: [
        .executable(
            name: "ASRPro",
            targets: ["ASRPro"]
        ),
    ],
    dependencies: [
        // Dependencies for backend API communication will be added here
        // .package(url: "https://github.com/Alamofire/Alamofire.git", from: "5.6.0"),
    ],
    targets: [
        .executableTarget(
            name: "ASRPro",
            dependencies: [],
            path: "ASRPro"
        ),
        .testTarget(
            name: "ASRProTests",
            dependencies: ["ASRPro"],
            path: "ASRProTests"
        ),
    ]
)