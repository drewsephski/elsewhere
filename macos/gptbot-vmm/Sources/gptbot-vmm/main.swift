import Foundation
import GptBotVMMLib

enum CLI {
    static func run() {
        let args = CommandLine.arguments
        guard args.count >= 3, args[1] == "serve" else {
            fputs("Usage: gptbot-vmm serve --config <path> --socket <path>\n", stderr)
            exit(2)
        }

        var configPath: String?
        var socketPath: String?
        var index = 2
        while index < args.count {
            switch args[index] {
            case "--config":
                index += 1
                if index < args.count { configPath = args[index] }
            case "--socket":
                index += 1
                if index < args.count { socketPath = args[index] }
            default:
                break
            }
            index += 1
        }

        guard let configPath, let socketPath else {
            fputs("Missing --config or --socket\n", stderr)
            exit(2)
        }

        do {
            let config = try VMConfigFile.load(from: URL(fileURLWithPath: configPath))
            let vm = LinuxVirtualMachine(config: config)
            let server = ControlServer(socketPath: URL(fileURLWithPath: socketPath), vm: vm)
            try server.run()
        } catch {
            fputs("gptbot-vmm failed: \(error)\n", stderr)
            exit(1)
        }
    }
}

CLI.run()
