import Darwin
import Foundation

public struct ControlRequest: Codable {
    public var cmd: String
    public var payload: String?
    public var timeoutSeconds: Double?
}

public struct ControlResponse: Codable {
    public var ok: Bool
    public var error: String?
    public var status: VMRuntimeStatus?
    public var result: String?

    public static func success(status: VMRuntimeStatus? = nil, result: String? = nil) -> ControlResponse {
        ControlResponse(ok: true, error: nil, status: status, result: result)
    }

    public static func failure(_ message: String) -> ControlResponse {
        ControlResponse(ok: false, error: message, status: nil, result: nil)
    }
}

public final class ControlServer {
    private let socketPath: URL
    private let vm: LinuxVirtualMachine
    private var listener: FileHandle?
    private var shouldRun = true

    public init(socketPath: URL, vm: LinuxVirtualMachine) {
        self.socketPath = socketPath
        self.vm = vm
    }

    public func run() throws {
        let path = socketPath.path
        if FileManager.default.fileExists(atPath: path) {
            try FileManager.default.removeItem(atPath: path)
        }

        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else {
            throw VMMError.bridgeUnavailable("Failed to create unix socket")
        }

        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        let pathData = path.utf8CString
        guard pathData.count <= MemoryLayout.size(ofValue: addr.sun_path) else {
            close(fd)
            throw VMMError.invalidRequest("Socket path too long")
        }
        withUnsafeMutablePointer(to: &addr.sun_path) { ptr in
            ptr.withMemoryRebound(to: CChar.self, capacity: pathData.count) { dest in
                for (index, byte) in pathData.enumerated() {
                    dest[index] = byte
                }
            }
        }

        let bindResult = withUnsafePointer(to: &addr) { ptr in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) { sockaddrPtr in
                bind(fd, sockaddrPtr, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard bindResult == 0 else {
            close(fd)
            throw VMMError.bridgeUnavailable("Failed to bind control socket")
        }

        guard listen(fd, 8) == 0 else {
            close(fd)
            throw VMMError.bridgeUnavailable("Failed to listen on control socket")
        }

        listener = FileHandle(fileDescriptor: fd, closeOnDealloc: true)

        while shouldRun {
            let clientFd = accept(fd, nil, nil)
            if clientFd < 0 {
                continue
            }
            handleClient(fd: clientFd)
            close(clientFd)
        }
    }

    public func stop() {
        shouldRun = false
    }

    private func handleClient(fd: Int32) {
        var tv = timeval(tv_sec: 120, tv_usec: 0)
        _ = withUnsafePointer(to: &tv) { ptr in
            setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, ptr, socklen_t(MemoryLayout<timeval>.size))
        }

        var buffer = Data()
        var chunk = [UInt8](repeating: 0, count: 65536)
        while true {
            let n = read(fd, &chunk, chunk.count)
            if n <= 0 {
                break
            }
            buffer.append(contentsOf: chunk.prefix(n))
            if buffer.contains(0x0A) {
                break
            }
        }

        guard let line = String(data: buffer, encoding: .utf8)?.trimmingCharacters(in: .newlines),
              !line.isEmpty,
              let data = line.data(using: .utf8)
        else {
            writeResponse(fd: fd, response: ControlResponse.failure("Invalid request"))
            return
        }

        let request: ControlRequest
        do {
            request = try JSONDecoder().decode(ControlRequest.self, from: data)
        } catch {
            writeResponse(fd: fd, response: ControlResponse.failure("JSON decode error: \(error.localizedDescription)"))
            return
        }

        let response = dispatch(request)
        writeResponse(fd: fd, response: response)
    }

    private func dispatch(_ request: ControlRequest) -> ControlResponse {
        let timeout = request.timeoutSeconds ?? 30.0
        switch request.cmd {
        case "status":
            return .success(status: vm.currentStatus())
        case "start":
            do {
                try vm.start()
                return .success(status: vm.currentStatus())
            } catch let error as VMMError {
                return .failure(error.description)
            } catch {
                return .failure(error.localizedDescription)
            }
        case "stop":
            do {
                try vm.stop()
                return .success(status: vm.currentStatus())
            } catch let error as VMMError {
                return .failure(error.description)
            } catch {
                return .failure(error.localizedDescription)
            }
        case "guest":
            guard let payload = request.payload else {
                return .failure("guest command requires payload")
            }
            do {
                let result = try vm.guestRequest(jsonLine: payload, timeout: timeout)
                return .success(status: vm.currentStatus(), result: result)
            } catch let error as VMMError {
                return .failure(error.description)
            } catch {
                return .failure(error.localizedDescription)
            }
        case "wait_guest":
            do {
                try vm.waitForGuestAgent(timeout: timeout)
                return .success(status: vm.currentStatus())
            } catch let error as VMMError {
                return .failure(error.description)
            } catch {
                return .failure(error.localizedDescription)
            }
        default:
            return .failure("Unknown command: \(request.cmd)")
        }
    }

    private func writeResponse(fd: Int32, response: ControlResponse) {
        do {
            let data = try JSONEncoder().encode(response)
            var line = data
            line.append(0x0A)
            _ = line.withUnsafeBytes { raw in
                write(fd, raw.baseAddress, raw.count)
            }
        } catch {
            let fallback = "{\"ok\":false,\"error\":\"encode failed\"}\n"
            _ = fallback.withCString { write(fd, $0, strlen($0)) }
        }
    }
}
