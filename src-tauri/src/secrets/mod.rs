use crate::error::AppError;

const SERVICE: &str = "com.drewsepeczi.gptbot";
const OPENAI_ACCOUNT: &str = "openai_api_key";

pub trait SecretStore: Send + Sync {
    fn get_openai_api_key(&self) -> Result<Option<String>, AppError>;
    fn set_openai_api_key(&self, key: &str) -> Result<(), AppError>;
    fn delete_openai_api_key(&self) -> Result<(), AppError>;
    fn has_openai_api_key(&self) -> Result<bool, AppError> {
        Ok(self.get_openai_api_key()?.is_some())
    }
}

pub struct KeychainSecretStore;

impl SecretStore for KeychainSecretStore {
    fn get_openai_api_key(&self) -> Result<Option<String>, AppError> {
        let entry = keyring::Entry::new(SERVICE, OPENAI_ACCOUNT)
            .map_err(|e| AppError::Secret(e.to_string()))?;
        match entry.get_password() {
            Ok(key) => {
                let trimmed = key.trim();
                if trimmed.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(trimmed.to_string()))
                }
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(AppError::Secret(e.to_string())),
        }
    }

    fn set_openai_api_key(&self, key: &str) -> Result<(), AppError> {
        let trimmed = key.trim();
        if trimmed.is_empty() {
            return Err(AppError::Validation("API key cannot be empty".into()));
        }
        let entry = keyring::Entry::new(SERVICE, OPENAI_ACCOUNT)
            .map_err(|e| AppError::Secret(e.to_string()))?;
        entry
            .set_password(trimmed)
            .map_err(|e| AppError::Secret(e.to_string()))?;
        Ok(())
    }

    fn delete_openai_api_key(&self) -> Result<(), AppError> {
        let entry = keyring::Entry::new(SERVICE, OPENAI_ACCOUNT)
            .map_err(|e| AppError::Secret(e.to_string()))?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(AppError::Secret(e.to_string())),
        }
    }
}

#[cfg(test)]
pub struct InMemorySecretStore {
    key: parking_lot::Mutex<Option<String>>,
}

#[cfg(test)]
impl InMemorySecretStore {
    pub fn new() -> Self {
        Self {
            key: parking_lot::Mutex::new(None),
        }
    }
}

#[cfg(test)]
impl SecretStore for InMemorySecretStore {
    fn get_openai_api_key(&self) -> Result<Option<String>, AppError> {
        Ok(self.key.lock().clone())
    }

    fn set_openai_api_key(&self, key: &str) -> Result<(), AppError> {
        let trimmed = key.trim();
        if trimmed.is_empty() {
            return Err(AppError::Validation("API key cannot be empty".into()));
        }
        *self.key.lock() = Some(trimmed.to_string());
        Ok(())
    }

    fn delete_openai_api_key(&self) -> Result<(), AppError> {
        *self.key.lock() = None;
        Ok(())
    }
}
