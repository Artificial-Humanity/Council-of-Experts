// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "PanelOfExpertsKit",
    platforms: [
        .macOS(.v14), .iOS(.v17)
    ],
    products: [
        .library(name: "PanelOfExpertsKit", targets: ["PanelOfExpertsKit"]),
        .executable(name: "PanelOfExpertsApp", targets: ["PanelOfExpertsApp"])
    ],
    dependencies: [],
    targets: [
        // FFI Binary target
        .binaryTarget(
            name: "panel_of_experts_ffiFFI",
            path: "panel_of_experts_ffiFFI.xcframework"
        ),
        
        // Swift wrapper module
        .target(
            name: "PanelOfExpertsKit",
            dependencies: [
                "panel_of_experts_ffiFFI"
            ],
            path: "Sources/Kit"
        ),
        
        // SwiftUI App executable
        .executableTarget(
            name: "PanelOfExpertsApp",
            dependencies: [
                "PanelOfExpertsKit",
                "panel_of_experts_ffiFFI"
            ],
            path: "Sources/App"
        )
    ]
)
