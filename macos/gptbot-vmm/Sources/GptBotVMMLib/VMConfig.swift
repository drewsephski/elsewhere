import Foundation

public struct VMConfigFile: Codable, Sendable {
    public var id: String
    public var hostArch: String
    public var guestArch: String
    public var cpuCount: Int
    public var memoryMib: Int
    public var kernelPath: String
    public var initrdPath: String?
    public var diskPath: String
    public var kernelCommandLine: String
    public var guestAgentPort: UInt32
    public var consoleLogPath: String

    public init(
        id: String,
        hostArch: String,
        guestArch: String,
        cpuCount: Int,
        memoryMib: Int,
        kernelPath: String,
        initrdPath: String?,
        diskPath: String,
        kernelCommandLine: String,
        guestAgentPort: UInt32,
        consoleLogPath: String = ""
    ) {
        self.id = id
        self.hostArch = hostArch
        self.guestArch = guestArch
        self.cpuCount = cpuCount
        self.memoryMib = memoryMib
        self.kernelPath = kernelPath
        self.initrdPath = initrdPath
        self.diskPath = diskPath
        self.kernelCommandLine = kernelCommandLine
        self.guestAgentPort = guestAgentPort
        self.consoleLogPath = consoleLogPath
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(String.self, forKey: .id)
        hostArch = try container.decode(String.self, forKey: .hostArch)
        guestArch = try container.decode(String.self, forKey: .guestArch)
        cpuCount = try container.decode(Int.self, forKey: .cpuCount)
        memoryMib = try container.decode(Int.self, forKey: .memoryMib)
        kernelPath = try container.decode(String.self, forKey: .kernelPath)
        initrdPath = try container.decodeIfPresent(String.self, forKey: .initrdPath)
        diskPath = try container.decode(String.self, forKey: .diskPath)
        kernelCommandLine = try container.decode(String.self, forKey: .kernelCommandLine)
        guestAgentPort = try container.decode(UInt32.self, forKey: .guestAgentPort)
        consoleLogPath = try container.decodeIfPresent(String.self, forKey: .consoleLogPath) ?? ""
    }

    public static func load(from url: URL) throws -> VMConfigFile {
        let data = try Data(contentsOf: url)
        return try JSONDecoder().decode(VMConfigFile.self, from: data)
    }

    public func resolvedConsoleLogPath() -> String {
        if !consoleLogPath.isEmpty {
            return consoleLogPath
        }
        let diskURL = URL(fileURLWithPath: diskPath)
        return diskURL.deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("logs/console.log")
            .path
    }
}
