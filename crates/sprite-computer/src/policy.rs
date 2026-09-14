use crate::types::{NetworkPolicyBody, NetworkPolicyRuleBody};

/// Default-deny outbound network for newly created Sprites.
pub fn default_deny_network_policy() -> NetworkPolicyConfig {
    NetworkPolicyConfig {
        rules: vec![NetworkPolicyRuleBody {
            domain: Some("*".into()),
            action: Some("deny".into()),
            include: None,
        }],
    }
}

/// Allow outbound HTTPS/HTTP for headless browser workloads inside the Sprite.
pub fn browser_workload_network_policy() -> NetworkPolicyConfig {
    NetworkPolicyConfig {
        rules: vec![NetworkPolicyRuleBody {
            domain: Some("*".into()),
            action: Some("allow".into()),
            include: None,
        }],
    }
}

#[derive(Debug, Clone)]
pub struct NetworkPolicyConfig {
    pub rules: Vec<NetworkPolicyRuleBody>,
}

impl NetworkPolicyConfig {
    pub fn to_body(&self) -> NetworkPolicyBody {
        NetworkPolicyBody {
            rules: self.rules.clone(),
        }
    }
}

fn rule_key(rule: &NetworkPolicyRuleBody) -> (Option<String>, Option<String>) {
    (
        rule.domain.as_ref().map(|d| d.to_ascii_lowercase()),
        rule.action.as_ref().map(|a| a.to_ascii_lowercase()),
    )
}

/// Compare provider-reported policy to the expected baseline (e.g. default-deny).
pub fn network_policy_matches(actual: &NetworkPolicyBody, expected: &NetworkPolicyConfig) -> bool {
    let expected_body = expected.to_body();
    if actual.rules.len() != expected_body.rules.len() {
        return false;
    }
    actual
        .rules
        .iter()
        .zip(expected_body.rules.iter())
        .all(|(left, right)| rule_key(left) == rule_key(right))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_deny_matches_itself() {
        let policy = default_deny_network_policy();
        assert!(network_policy_matches(&policy.to_body(), &policy));
    }
}
