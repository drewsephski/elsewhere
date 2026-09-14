/// Authoritative default model for Elsewhere (Luna).
pub const DEFAULT_MODEL: &str = "gpt-5.6-luna";

/// Returns the canonical Luna model id when present in `available`, otherwise `None`.
pub fn luna_model_id(available: &[impl AsRef<str>]) -> Option<String> {
    let preferred = available
        .iter()
        .find(|id| {
            let lower = id.as_ref().to_lowercase();
            lower == DEFAULT_MODEL || lower.contains("luna")
        })
        .map(|id| id.as_ref().to_string());
    preferred
}

/// Default model selection policy: Luna if listed, else explicit default id (caller surfaces availability).
pub fn default_model_id(available: &[impl AsRef<str>]) -> String {
    luna_model_id(available).unwrap_or_else(|| DEFAULT_MODEL.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_model_constant_is_luna() {
        assert_eq!(DEFAULT_MODEL, "gpt-5.6-luna");
    }

    #[test]
    fn selects_luna_when_present() {
        let models = ["gpt-4o-mini", "gpt-5.6-luna", "o3-mini"];
        assert_eq!(luna_model_id(&models).as_deref(), Some("gpt-5.6-luna"));
    }

    #[test]
    fn does_not_silently_pick_first_unrelated_model() {
        let models = ["gpt-4o-mini", "o3-mini"];
        assert!(luna_model_id(&models).is_none());
        assert_eq!(default_model_id(&models), DEFAULT_MODEL);
    }
}
