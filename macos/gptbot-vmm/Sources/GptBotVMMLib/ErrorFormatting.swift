import Foundation

enum ErrorFormatting {
    static func describe(_ error: Error) -> String {
        let ns = error as NSError
        var parts: [String] = [
            "domain=\(ns.domain)",
            "code=\(ns.code)",
            "description=\(ns.localizedDescription)",
        ]
        if let reason = ns.localizedFailureReason, !reason.isEmpty {
            parts.append("reason=\(reason)")
        }
        if !ns.userInfo.isEmpty {
            let info = ns.userInfo
                .map { "\($0.key)=\($0.value)" }
                .sorted()
                .joined(separator: ", ")
            parts.append("userInfo={\(info)}")
        }
        return parts.joined(separator: "; ")
    }

    static func tailOfFile(at path: String, maxBytes: Int = 8192) -> String? {
        guard let data = FileManager.default.contents(atPath: path), !data.isEmpty else {
            return nil
        }
        let start = max(0, data.count - maxBytes)
        let slice = data.subdata(in: start..<data.count)
        return String(data: slice, encoding: .utf8)
    }
}
