use parking_lot::Mutex;
use std::fs;
use std::sync::Arc;
use std::time::Duration;

use super::paths::VmLayout;
use super::protocol::{GuestRequest, GuestResponse, VmConfigFile, VmInfo, VmLifecycleState};
use super::provision;
use super::vmm_client::{self, VmmProcess};

#[derive(Debug)]
enum InternalState {
    NotCreated,
    Stopped,
    Starting,
    Running,
    Stopping,
    Error(String),
}

pub struct VirtualMachineManager {
    layout: VmLayout,
    state: Mutex<InternalState>,
    vmm: Mutex<Option<VmmProcess>>,
    last_vmm_binary: Mutex<Option<String>>,
}

impl VirtualMachineManager {
    pub fn new(app_data_dir: &std::path::Path) -> Self {
        let layout = VmLayout::new(app_data_dir);
        let initial = if layout.vm_config_path().exists() {
            InternalState::Stopped
        } else {
            InternalState::NotCreated
        };
        Self {
            layout,
            state: Mutex::new(initial),
            vmm: Mutex::new(None),
            last_vmm_binary: Mutex::new(None),
        }
    }

    pub fn layout(&self) -> &VmLayout {
        &self.layout
    }

    pub fn info(&self) -> Result<VmInfo, String> {
        let created = self.layout.vm_config_path().exists();
        let disk_path = self.layout.disk_path();
        let disk_bytes = if disk_path.exists() {
            fs::metadata(&disk_path).map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };

        let (state, message, guest_bridge_ready) = self.read_runtime_state(created)?;

        let (cpu, memory, guest_arch) = if created {
            let config = self.load_config()?;
            (config.cpu_count, config.memory_mib, config.guest_arch)
        } else {
            (2, 2048, provision::guest_arch().into())
        };

        let vmm_binary_path = self.last_vmm_binary.lock().clone();

        Ok(VmInfo {
            state,
            host_arch: provision::host_arch().into(),
            guest_arch,
            cpu_count: cpu,
            memory_mib: memory,
            disk_path: disk_path.to_string_lossy().into(),
            disk_bytes,
            config_path: self.layout.vm_config_path().to_string_lossy().into(),
            guest_bridge_ready,
            message,
            created,
            console_log_path: self.layout.console_log_path().to_string_lossy().into(),
            vmm_binary_path,
        })
    }

    fn read_runtime_state(
        &self,
        created: bool,
    ) -> Result<(VmLifecycleState, Option<String>, bool), String> {
        if !created {
            return Ok((VmLifecycleState::NotCreated, None, false));
        }

        let locked = self.state.lock();
        match &*locked {
            InternalState::NotCreated => Ok((VmLifecycleState::Stopped, None, false)),
            InternalState::Stopped => Ok((VmLifecycleState::Stopped, None, false)),
            InternalState::Starting => Ok((VmLifecycleState::Starting, None, false)),
            InternalState::Running => {
                if let Ok(resp) = vmm_client::control_request(
                    &self.layout.control_socket_path(),
                    "status",
                    None,
                    Duration::from_secs(2),
                ) {
                    let ready = resp
                        .status
                        .map(|s| s.guest_bridge_ready)
                        .unwrap_or(false);
                    return Ok((VmLifecycleState::Running, None, ready));
                }
                Ok((VmLifecycleState::Running, None, false))
            }
            InternalState::Stopping => Ok((VmLifecycleState::Stopping, None, false)),
            InternalState::Error(msg) => Ok((
                VmLifecycleState::Error,
                Some(msg.clone()),
                false,
            )),
        }
    }

    pub fn provision(&self) -> Result<VmInfo, String> {
        provision::provision_vm(&self.layout)?;
        *self.state.lock() = InternalState::Stopped;
        self.info()
    }

    pub fn start(&self) -> Result<VmInfo, String> {
        if !self.layout.vm_config_path().exists() {
            return Err("VM not provisioned. Create the local computer first.".into());
        }

        {
            let mut state = self.state.lock();
            if matches!(*state, InternalState::Running | InternalState::Starting) {
                return Err("VM is already running or starting".into());
            }
            *state = InternalState::Starting;
        }

        let result = self.start_inner();
        if result.is_err() {
            *self.state.lock() = InternalState::Error(
                result
                    .as_ref()
                    .err()
                    .cloned()
                    .unwrap_or_else(|| "start failed".into()),
            );
        }
        result
    }

    fn start_inner(&self) -> Result<VmInfo, String> {
        let vmm_binary = vmm_client::locate_vmm_binary()?;
        let config_path = self.layout.vm_config_path();
        let socket_path = self.layout.control_socket_path();
        let log_path = self.layout.vmm_log_path();

        self.layout.ensure_directories().map_err(|e| e.to_string())?;

        let mut vmm_guard = self.vmm.lock();
        if let Some(existing) = vmm_guard.as_mut() {
            existing.stop();
        }

        let process = VmmProcess::spawn(&vmm_binary, &config_path, &socket_path, &log_path)?;
        *self.last_vmm_binary.lock() = Some(vmm_binary.to_string_lossy().into());
        *vmm_guard = Some(process);

        let response = vmm_client::control_request(
            &socket_path,
            "start",
            None,
            Duration::from_secs(60),
        );

        let response = match response {
            Ok(resp) => resp,
            Err(err) => {
                if let Some(mut process) = vmm_guard.take() {
                    process.stop();
                }
                *self.state.lock() = InternalState::Stopped;
                return Err(err);
            }
        };

        if !response.ok {
            let base = response
                .error
                .unwrap_or_else(|| "VMM start command failed".into());
            let console = self.layout.console_log_path();
            let tail = vmm_client::tail_file(&console, 8192);
            let mut message = format!(
                "{base}\nVMM binary: {}\nGuest console log: {}",
                vmm_binary.display(),
                console.display()
            );
            if let Some(tail) = tail {
                if !tail.trim().is_empty() {
                    message.push_str("\n--- console.log (tail) ---\n");
                    message.push_str(&tail);
                }
            }
            return Err(message);
        }

        *self.state.lock() = InternalState::Running;
        self.info()
    }

    pub fn stop(&self) -> Result<VmInfo, String> {
        {
            let state = self.state.lock();
            if matches!(*state, InternalState::Stopped | InternalState::NotCreated) {
                return self.info();
            }
        }

        *self.state.lock() = InternalState::Stopping;

        let socket_path = self.layout.control_socket_path();
        if socket_path.exists() {
            let _ = vmm_client::control_request(&socket_path, "stop", None, Duration::from_secs(60));
        }

        if let Some(mut process) = self.vmm.lock().take() {
            process.stop();
        }

        *self.state.lock() = InternalState::Stopped;
        self.info()
    }

    pub fn restart(&self) -> Result<VmInfo, String> {
        self.stop()?;
        self.start()
    }

    pub fn guest_request(&self, request: GuestRequest) -> Result<GuestResponse, String> {
        {
            let state = self.state.lock();
            if !matches!(*state, InternalState::Running) {
                return Err("VM is not running".into());
            }
        }

        let payload = serde_json::to_string(&request).map_err(|e| e.to_string())?;
        let line = format!("{payload}\n");
        let response = vmm_client::control_request(
            &self.layout.control_socket_path(),
            "guest",
            Some(line),
            Duration::from_secs(60),
        )?;

        if !response.ok {
            return Err(response
                .error
                .unwrap_or_else(|| "guest bridge request failed".into()));
        }

        let result = response
            .result
            .ok_or_else(|| "missing guest response body".to_string())?;
        vmm_client::parse_guest_response(&result)
    }

    pub fn wait_for_guest(&self, timeout: Duration) -> Result<VmInfo, String> {
        {
            let state = self.state.lock();
            if !matches!(*state, InternalState::Running | InternalState::Starting) {
                return Err("VM is not running".into());
            }
        }

        let response = vmm_client::control_request(
            &self.layout.control_socket_path(),
            "wait_guest",
            None,
            timeout,
        )?;

        if !response.ok {
            let base = response
                .error
                .unwrap_or_else(|| "guest agent did not become ready".into());
            let console = self.layout.console_log_path();
            let tail = vmm_client::tail_file(&console, 8192);
            let mut message = format!(
                "{base}\nGuest console log: {}",
                console.display()
            );
            if let Some(tail) = tail {
                if !tail.trim().is_empty() {
                    message.push_str("\n--- console.log (tail) ---\n");
                    message.push_str(&tail);
                }
            }
            return Err(message);
        }

        *self.state.lock() = InternalState::Running;
        self.info()
    }

    pub fn guest_health(&self) -> Result<bool, String> {
        let response = self.guest_request(GuestRequest {
            id: uuid::Uuid::new_v4().to_string(),
            method: "ping".into(),
            params: serde_json::json!({}),
        })?;
        Ok(response.ok)
    }

    pub fn ensure_guest_ready(&self, timeout: Duration) -> Result<VmInfo, String> {
        if !self.layout.vm_config_path().exists() {
            return Err("VM not provisioned. Create the local computer first.".into());
        }

        let needs_start = {
            let state = self.state.lock();
            matches!(
                *state,
                InternalState::NotCreated | InternalState::Stopped | InternalState::Error(_)
            )
        };

        if needs_start {
            self.start()?;
        }

        let info = self.info()?;
        if info.guest_bridge_ready {
            return Ok(info);
        }
        self.wait_for_guest(timeout)
    }

    fn load_config(&self) -> Result<VmConfigFile, String> {
        let data = fs::read_to_string(self.layout.vm_config_path()).map_err(|e| e.to_string())?;
        serde_json::from_str(&data).map_err(|e| e.to_string())
    }
}

pub type SharedVirtualMachineManager = Arc<VirtualMachineManager>;
