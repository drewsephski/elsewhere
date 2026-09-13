import Foundation

public enum VMMError: Error, CustomStringConvertible {
    case unsupported(String)
    case missingArtifact(String)
    case invalidState(String)
    case invalidRequest(String)
    case bootFailed(String)
    case bridgeUnavailable(String)

    public var description: String {
        switch self {
        case .unsupported(let msg): return msg
        case .missingArtifact(let msg): return msg
        case .invalidState(let msg): return msg
        case .invalidRequest(let msg): return msg
        case .bootFailed(let msg): return msg
        case .bridgeUnavailable(let msg): return msg
        }
    }
}
