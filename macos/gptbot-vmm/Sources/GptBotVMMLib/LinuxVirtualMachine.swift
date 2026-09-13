import Foundation
import Virtualization

public final class LinuxVirtualMachine: NSObject, @unchecked Sendable {
    private let config: VMConfigFile
    private let queue = DispatchQueue(label: "com.gptbot.vmm.vm")
    private var virtualMachine: VZVirtualMachine?
    private var socketDevice: VZVirtioSocketDevice?
    private let statusLock = NSLock()
    private var _status = VMRuntimeStatus(phase: .stopped)
    private var guestReady = false

    public init(config: VMConfigFile) {
        self.config = config
        super.init()
    }

    public func currentStatus() -> VMRuntimeStatus {
        statusLock.lock()
        defer { statusLock.unlock() }
        var s = _status
        s.guestBridgeReady = guestReady
        return s
    }

    private func setStatus(_ phase: VMRuntimePhase, message: String? = nil) {
        statusLock.lock()
        _status = VMRuntimeStatus(phase: phase, message: message, guestBridgeReady: guestReady)
        statusLock.unlock()
    }

    public func start() throws {
        let current = currentStatus()
        if current.phase == .running || current.phase == .starting {
            throw VMMError.invalidState("VM is already running or starting")
        }
        setStatus(.starting)

        guard VZVirtualMachine.isSupported else {
            setStatus(.error, message: "Virtualization is not supported on this Mac")
            throw VMMError.unsupported("Virtualization.framework is not supported")
        }

        let semaphore = DispatchSemaphore(value: 0)
        var startError: Error?

        queue.async { [weak self] in
            guard let self else {
                semaphore.signal()
                return
            }
            do {
                let vmConfig = try self.makeConfiguration()
                let vm = VZVirtualMachine(configuration: vmConfig, queue: self.queue)
                vm.delegate = self
                self.virtualMachine = vm
                self.socketDevice = vm.socketDevices.first as? VZVirtioSocketDevice
                vm.start { result in
                    if case .failure(let error) = result {
                        startError = error
                    }
                    semaphore.signal()
                }
            } catch {
                startError = error
                semaphore.signal()
            }
        }
        semaphore.wait()

        if let startError {
            setStatus(.error, message: startError.localizedDescription)
            throw VMMError.bootFailed(startError.localizedDescription)
        }

        setStatus(.running)
        guestReady = false
    }

    public func stop() throws {
        guard let vm = virtualMachine else {
            setStatus(.stopped)
            return
        }
        let current = currentStatus()
        if current.phase == .stopped || current.phase == .stopping {
            return
        }
        setStatus(.stopping)

        let semaphore = DispatchSemaphore(value: 0)
        queue.async { [weak self] in
            vm.stop { _ in
                self?.virtualMachine = nil
                self?.socketDevice = nil
                self?.guestReady = false
                self?.setStatus(.stopped)
                semaphore.signal()
            }
        }
        semaphore.wait()
    }

    private func makeConfiguration() throws -> VZVirtualMachineConfiguration {
        let kernelURL = URL(fileURLWithPath: config.kernelPath)
        guard FileManager.default.fileExists(atPath: kernelURL.path) else {
            throw VMMError.missingArtifact("kernel not found at \(config.kernelPath)")
        }
        let bootLoader = VZLinuxBootLoader(kernelURL: kernelURL)

        if let initrd = config.initrdPath {
            let initrdURL = URL(fileURLWithPath: initrd)
            guard FileManager.default.fileExists(atPath: initrdURL.path) else {
                throw VMMError.missingArtifact("initrd not found at \(initrd)")
            }
            bootLoader.initialRamdiskURL = initrdURL
        }

        bootLoader.commandLine = config.kernelCommandLine

        let vmConfig = VZVirtualMachineConfiguration()
        vmConfig.bootLoader = bootLoader
        vmConfig.cpuCount = config.cpuCount
        vmConfig.memorySize = UInt64(config.memoryMib) * 1024 * 1024

        let platform = VZGenericPlatformConfiguration()
        vmConfig.platform = platform

        let diskURL = URL(fileURLWithPath: config.diskPath)
        guard FileManager.default.fileExists(atPath: diskURL.path) else {
            throw VMMError.missingArtifact("disk not found at \(config.diskPath)")
        }
        let diskAttachment = try VZDiskImageStorageDeviceAttachment(
            url: diskURL,
            readOnly: false
        )
        let disk = VZVirtioBlockDeviceConfiguration(attachment: diskAttachment)
        vmConfig.storageDevices = [disk]

        let network = VZVirtioNetworkDeviceConfiguration()
        network.attachment = VZNATNetworkDeviceAttachment()
        vmConfig.networkDevices = [network]

        let entropy = VZVirtioEntropyDeviceConfiguration()
        vmConfig.entropyDevices = [entropy]

        let socketConfig = VZVirtioSocketDeviceConfiguration()
        vmConfig.socketDevices = [socketConfig]

        let serialPort = VZVirtioConsoleDeviceSerialPortConfiguration()
        vmConfig.serialPorts = [serialPort]

        try vmConfig.validate()
        return vmConfig
    }

    public func waitForGuestAgent(timeout: TimeInterval) throws {
        guard let socketDevice else {
            throw VMMError.bridgeUnavailable("Virtio socket device is not available")
        }
        let port = config.guestAgentPort
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if try pingGuest(socketDevice: socketDevice, port: port) {
                guestReady = true
                statusLock.lock()
                _status.guestBridgeReady = true
                statusLock.unlock()
                return
            }
            Thread.sleep(forTimeInterval: 0.5)
        }
        throw VMMError.bridgeUnavailable("Guest agent did not become ready within \(timeout)s")
    }

    public func guestRequest(jsonLine: String, timeout: TimeInterval) throws -> String {
        guard let socketDevice else {
            throw VMMError.bridgeUnavailable("Virtio socket device is not available")
        }
        let port = config.guestAgentPort
        return try GuestBridge.request(
            socketDevice: socketDevice,
            port: port,
            payload: jsonLine,
            timeout: timeout
        )
    }

    private func pingGuest(socketDevice: VZVirtioSocketDevice, port: UInt32) throws -> Bool {
        let ping = "{\"id\":\"health\",\"method\":\"ping\"}\n"
        do {
            let response = try GuestBridge.request(
                socketDevice: socketDevice,
                port: port,
                payload: ping,
                timeout: 2.0
            )
            return response.contains("\"ok\":true")
        } catch {
            return false
        }
    }
}

extension LinuxVirtualMachine: VZVirtualMachineDelegate {
    public func guestDidStop(_ virtualMachine: VZVirtualMachine) {
        setStatus(.stopped, message: "Guest stopped unexpectedly")
        guestReady = false
        self.virtualMachine = nil
        self.socketDevice = nil
    }

    public func virtualMachine(_ virtualMachine: VZVirtualMachine, didStopWithError error: Error) {
        setStatus(.error, message: error.localizedDescription)
        guestReady = false
        self.virtualMachine = nil
        self.socketDevice = nil
    }
}
