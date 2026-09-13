import Darwin
import Foundation
import Virtualization

enum GuestBridge {
    static func request(
        socketDevice: VZVirtioSocketDevice,
        port: UInt32,
        payload: String,
        timeout: TimeInterval
    ) throws -> String {
        let semaphore = DispatchSemaphore(value: 0)
        var connection: VZVirtioSocketConnection?
        var connectError: Error?

        socketDevice.connect(toPort: port) { result in
            switch result {
            case .success(let conn):
                connection = conn
            case .failure(let error):
                connectError = error
            }
            semaphore.signal()
        }
        semaphore.wait()

        if let connectError {
            throw VMMError.bridgeUnavailable(connectError.localizedDescription)
        }
        guard let connection else {
            throw VMMError.bridgeUnavailable("Failed to open virtio socket connection")
        }

        guard let data = payload.data(using: .utf8) else {
            throw VMMError.invalidRequest("Payload is not valid UTF-8")
        }

        let fd = connection.fileDescriptor
        try setSocketTimeout(fd: fd, seconds: timeout)

        try writeAll(fd: fd, data: data)

        var received = Data()
        let bufferSize = 4096

        while true {
            var chunk = [UInt8](repeating: 0, count: bufferSize)
            let readCount = read(fd, &chunk, bufferSize)
            if readCount <= 0 {
                if readCount < 0 && errno == EAGAIN {
                    throw VMMError.bridgeUnavailable("Guest read timed out after \(timeout)s")
                }
                break
            }
            received.append(contentsOf: chunk.prefix(readCount))
            if received.contains(0x0A) {
                break
            }
        }

        connection.close()

        guard let line = String(data: received, encoding: .utf8)?.trimmingCharacters(in: .newlines),
              !line.isEmpty
        else {
            throw VMMError.bridgeUnavailable("Empty response from guest agent")
        }
        return line
    }

    private static func setSocketTimeout(fd: Int32, seconds: TimeInterval) throws {
        var tv = timeval(
            tv_sec: __darwin_time_t(max(1, Int(seconds))),
            tv_usec: 0
        )
        let rcSend = withUnsafePointer(to: &tv) { ptr in
            setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, ptr, socklen_t(MemoryLayout<timeval>.size))
        }
        let rcRecv = withUnsafePointer(to: &tv) { ptr in
            setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, ptr, socklen_t(MemoryLayout<timeval>.size))
        }
        if rcSend != 0 || rcRecv != 0 {
            throw VMMError.bridgeUnavailable("Failed to set socket timeouts: \(String(cString: strerror(errno)))")
        }
    }

    private static func writeAll(fd: Int32, data: Data) throws {
        try data.withUnsafeBytes { raw in
            guard let base = raw.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                throw VMMError.invalidRequest("Invalid payload buffer")
            }
            var sent = 0
            while sent < data.count {
                let n = write(fd, base.advanced(by: sent), data.count - sent)
                if n < 0 {
                    throw VMMError.bridgeUnavailable("write failed: \(String(cString: strerror(errno)))")
                }
                if n == 0 {
                    break
                }
                sent += n
            }
        }
    }
}
