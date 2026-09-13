// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "gptbot-vmm",
    platforms: [.macOS(.v13)],
    products: [
        .executable(name: "gptbot-vmm", targets: ["gptbot-vmm"]),
    ],
    targets: [
        .executableTarget(
            name: "gptbot-vmm",
            dependencies: ["GptBotVMMLib"]
        ),
        .target(name: "GptBotVMMLib"),
    ]
)
