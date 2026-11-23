// swift-tools-version: 5.9
import PackageDescription
let package = Package(name: "OmenNative", platforms: [.iOS(.v15)], products: [.library(name: "OmenNative", targets: ["OmenNative"])], targets: [
    .target(name: "COmenBridge", path: "Sources/COmenBridge", publicHeadersPath: "include"),
    .target(name: "OmenNative", dependencies: ["COmenBridge"], linkerSettings: [.linkedLibrary("omen_plugin_ffi")])
])
