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
        guestAgentPort: UInt32
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
    }

    public static func load(from url: URL) throws -> VMConfigFile {
        let data = try Data(contentsOf: url)
        return try JSONDecoder().decode(VMConfigFile.self, from: data)
    }
}
