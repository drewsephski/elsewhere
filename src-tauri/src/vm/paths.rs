use std::path::{Path, PathBuf};

/// Application-owned VM directory layout under macOS app data.
#[derive(Debug, Clone)]
pub struct VmLayout {
    pub root: PathBuf,
}

impl VmLayout {
    pub fn new(app_data_dir: &Path) -> Self {
        Self {
            root: app_data_dir.join("vm"),
        }
    }

    pub fn config_dir(&self) -> PathBuf {
        self.root.join("config")
    }

    pub fn disks_dir(&self) -> PathBuf {
        self.root.join("disks")
    }

    pub fn runtime_dir(&self) -> PathBuf {
        self.root.join("runtime")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    pub fn artifacts_dir(&self) -> PathBuf {
        self.root.join("artifacts")
    }

    pub fn vm_config_path(&self) -> PathBuf {
        self.config_dir().join("vm.json")
    }

    pub fn disk_path(&self) -> PathBuf {
        self.disks_dir().join("root.raw")
    }

    pub fn kernel_path(&self) -> PathBuf {
        self.artifacts_dir().join("vmlinuz-virt")
    }

    pub fn initrd_path(&self) -> PathBuf {
        self.artifacts_dir().join("initramfs-virt")
    }

    pub fn control_socket_path(&self) -> PathBuf {
        self.runtime_dir().join("vmm.sock")
    }

    pub fn vmm_log_path(&self) -> PathBuf {
        self.logs_dir().join("vmm.log")
    }

    pub fn ensure_directories(&self) -> std::io::Result<()> {
        for dir in [
            self.config_dir(),
            self.disks_dir(),
            self.runtime_dir(),
            self.logs_dir(),
            self.artifacts_dir(),
        ] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }
}
