use crate::db::Database;
use crate::secrets::SecretStore;
use crate::vm::SharedVirtualMachineManager;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub struct AppState {
    pub db: Arc<Mutex<Database>>,
    pub secrets: Arc<dyn SecretStore>,
    pub active_streams: Mutex<HashMap<String, Arc<AtomicBool>>>,
    #[cfg(target_os = "macos")]
    pub vm: SharedVirtualMachineManager,
}

impl AppState {
    pub fn register_cancel_token(&self, request_id: &str) -> Arc<AtomicBool> {
        let token = Arc::new(AtomicBool::new(false));
        self.active_streams
            .lock()
            .insert(request_id.to_string(), token.clone());
        token
    }

    pub fn cancel_request(&self, request_id: &str) -> bool {
        let map = self.active_streams.lock();
        if let Some(token) = map.get(request_id) {
            token.store(true, std::sync::atomic::Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    pub fn remove_cancel_token(&self, request_id: &str) {
        self.active_streams.lock().remove(request_id);
    }
}
