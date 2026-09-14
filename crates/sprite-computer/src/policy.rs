use crate::types::{NetworkPolicyRuleBody, NetworkPolicyBody};

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
