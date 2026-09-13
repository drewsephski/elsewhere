import Foundation

public enum VMRuntimePhase: String, Codable, Sendable {
    case stopped
    case starting
    case running
    case stopping
    case error
}

public struct VMRuntimeStatus: Codable, Sendable {
    public var phase: VMRuntimePhase
    public var message: String?
    public var guestBridgeReady: Bool

    public init(phase: VMRuntimePhase, message: String? = nil, guestBridgeReady: Bool = false) {
        self.phase = phase
        self.message = message
        self.guestBridgeReady = guestBridgeReady
    }
}
