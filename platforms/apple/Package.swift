// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "CouncilOfExpertsKit",
    platforms: [
        .macOS(.v14), .iOS(.v17)
    ],
    products: [
        .library(name: "CouncilOfExpertsKit", targets: ["CouncilOfExpertsKit"]),
        .executable(name: "CouncilOfExpertsApp", targets: ["CouncilOfExpertsApp"])
    ],
    dependencies: [],
    targets: [
        // FFI Binary target
        .binaryTarget(
            name: "council_of_experts_ffiFFI",
            path: "../../build/frameworks/council_of_experts_ffiFFI.xcframework"
        ),
        
        // Swift wrapper module
        .target(
            name: "CouncilOfExpertsKit",
            dependencies: [
                "council_of_experts_ffiFFI"
            ],
            path: "Sources/Kit"
        ),
        
        // SwiftUI App executable
        .executableTarget(
            name: "CouncilOfExpertsApp",
            dependencies: [
                "CouncilOfExpertsKit",
                "council_of_experts_ffiFFI"
            ],
            path: "Sources/App"
        )
    ]
)
