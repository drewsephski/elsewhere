/// Authoritative default model for Elsewhere (Luna).
pub const DEFAULT_MODEL: &str = "gpt-5.6-luna";

/// Product model a Bot can be assigned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BotModelSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub blurb: &'static str,
}

/// Curated ChatGPT / Codex models shown in Bot settings.
/// Luna stays the default; Sol, Terra, and Astra are opt-in per Bot.
pub const BOT_MODELS: &[BotModelSpec] = &[
    BotModelSpec {
        id: DEFAULT_MODEL,
        label: "5.6 Luna",
        blurb: "Fast and affordable. Best for clear, repeatable work.",
    },
    BotModelSpec {
        id: "gpt-5.6-terra",
        label: "5.6 Terra",
        blurb: "Balanced intelligence and cost for everyday work.",
    },
    BotModelSpec {
        id: "gpt-5.6-sol",
        label: "5.6 Sol",
        blurb: "Flagship GPT-5.6 for complex professional work.",
    },
    BotModelSpec {
        id: "gpt-6-astra",
        label: "Astra 6",
        blurb: "Strongest model for hard end-to-end work.",
    },
];

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

/// Trim, map product aliases, and default empty values to Luna.
/// Unknown ids pass through so existing Bots keep their stored model.
pub fn canonical_bot_model(raw: Option<&str>) -> String {
    let Some(value) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return DEFAULT_MODEL.to_string();
    };
    let key = value.to_ascii_lowercase().replace('_', "-");
    let key = key.replace(' ', "-");
    if let Some(model) = BOT_MODELS
        .iter()
        .find(|model| model.id.eq_ignore_ascii_case(&key))
    {
        return model.id.to_string();
    }
    match key.as_str() {
        "luna" | "5.6-luna" => DEFAULT_MODEL.to_string(),
        "terra" | "5.6-terra" => "gpt-5.6-terra".to_string(),
        "sol" | "5.6-sol" | "gpt-5.6" => "gpt-5.6-sol".to_string(),
        "astra" | "astra-6" | "gpt-6" | "6-astra" => "gpt-6-astra".to_string(),
        _ => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_model_constant_is_luna() {
        assert_eq!(DEFAULT_MODEL, "gpt-5.6-luna");
        assert_eq!(BOT_MODELS[0].id, DEFAULT_MODEL);
    }

    #[test]
    fn catalog_includes_astra_sol_and_terra() {
        let ids: Vec<&str> = BOT_MODELS.iter().map(|model| model.id).collect();
        assert_eq!(
            ids,
            [
                "gpt-5.6-luna",
                "gpt-5.6-terra",
                "gpt-5.6-sol",
                "gpt-6-astra"
            ]
        );
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

    #[test]
    fn canonical_model_defaults_empty_to_luna() {
        assert_eq!(canonical_bot_model(None), DEFAULT_MODEL);
        assert_eq!(canonical_bot_model(Some("")), DEFAULT_MODEL);
        assert_eq!(canonical_bot_model(Some("   ")), DEFAULT_MODEL);
    }

    #[test]
    fn canonical_model_maps_product_aliases() {
        assert_eq!(canonical_bot_model(Some("luna")), DEFAULT_MODEL);
        assert_eq!(canonical_bot_model(Some("5.6 Sol")), "gpt-5.6-sol");
        assert_eq!(canonical_bot_model(Some("gpt-5.6")), "gpt-5.6-sol");
        assert_eq!(canonical_bot_model(Some("Astra 6")), "gpt-6-astra");
        assert_eq!(canonical_bot_model(Some(" gpt-6-astra ")), "gpt-6-astra");
    }

    #[test]
    fn canonical_model_keeps_unknown_ids() {
        assert_eq!(canonical_bot_model(Some("gpt-4o-mini")), "gpt-4o-mini");
    }
}
