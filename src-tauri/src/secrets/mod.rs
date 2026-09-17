use crate::error::AppError;

const SERVICE: &str = "com.drewsepeczi.gptbot";
const OPENAI_ACCOUNT: &str = "openai_api_key";
const ELSEWHERE_DEVICE_ACCOUNT: &str = "elsewhere_device_credential";

pub trait SecretStore: Send + Sync {
    fn get_openai_api_key(&self) -> Result<Option<String>, AppError>;
    fn set_openai_api_key(&self, key: &str) -> Result<(), AppError>;
    fn delete_openai_api_key(&self) -> Result<(), AppError>;
    fn has_openai_api_key(&self) -> Result<bool, AppError> {
        Ok(self.get_openai_api_key()?.is_some())
    }
    fn get_elsewhere_device_credential(&self) -> Result<Option<String>, AppError>;
    fn set_elsewhere_device_credential(&self, credential: &str) -> Result<(), AppError>;
    fn delete_elsewhere_device_credential(&self) -> Result<(), AppError>;
}

pub struct KeychainSecretStore;

impl KeychainSecretStore {
    fn password(account: &str) -> Result<Option<String>, AppError> {
        let entry =
            keyring::Entry::new(SERVICE, account).map_err(|e| AppError::Secret(e.to_string()))?;
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

    fn set_password(account: &str, value: &str, empty_error: &str) -> Result<(), AppError> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(AppError::Validation(empty_error.into()));
        }
        let entry =
            keyring::Entry::new(SERVICE, account).map_err(|e| AppError::Secret(e.to_string()))?;
        entry
            .set_password(trimmed)
            .map_err(|e| AppError::Secret(e.to_string()))?;
        Ok(())
    }

    fn delete_password(account: &str) -> Result<(), AppError> {
        let entry =
            keyring::Entry::new(SERVICE, account).map_err(|e| AppError::Secret(e.to_string()))?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(AppError::Secret(e.to_string())),
        }
    }
}

impl SecretStore for KeychainSecretStore {
    fn get_openai_api_key(&self) -> Result<Option<String>, AppError> {
        Self::password(OPENAI_ACCOUNT)
    }

    fn set_openai_api_key(&self, key: &str) -> Result<(), AppError> {
        Self::set_password(OPENAI_ACCOUNT, key, "API key cannot be empty")
    }

    fn delete_openai_api_key(&self) -> Result<(), AppError> {
        Self::delete_password(OPENAI_ACCOUNT)
    }

    fn get_elsewhere_device_credential(&self) -> Result<Option<String>, AppError> {
        Self::password(ELSEWHERE_DEVICE_ACCOUNT)
    }

    fn set_elsewhere_device_credential(&self, credential: &str) -> Result<(), AppError> {
        Self::set_password(
            ELSEWHERE_DEVICE_ACCOUNT,
            credential,
            "device credential cannot be empty",
        )
    }

    fn delete_elsewhere_device_credential(&self) -> Result<(), AppError> {
        Self::delete_password(ELSEWHERE_DEVICE_ACCOUNT)
    }
}

#[cfg(test)]
pub struct InMemorySecretStore {
    key: parking_lot::Mutex<Option<String>>,
    device_credential: parking_lot::Mutex<Option<String>>,
}

#[cfg(test)]
impl InMemorySecretStore {
    pub fn new() -> Self {
        Self {
            key: parking_lot::Mutex::new(None),
            device_credential: parking_lot::Mutex::new(None),
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

    fn get_elsewhere_device_credential(&self) -> Result<Option<String>, AppError> {
        Ok(self.device_credential.lock().clone())
    }

    fn set_elsewhere_device_credential(&self, credential: &str) -> Result<(), AppError> {
        let trimmed = credential.trim();
        if trimmed.is_empty() {
            return Err(AppError::Validation(
                "device credential cannot be empty".into(),
            ));
        }
        *self.device_credential.lock() = Some(trimmed.to_string());
        Ok(())
    }

    fn delete_elsewhere_device_credential(&self) -> Result<(), AppError> {
        *self.device_credential.lock() = None;
        Ok(())
    }
}
