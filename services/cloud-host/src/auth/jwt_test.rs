//! ES256 test keys for integration tests (generated at runtime).

#[cfg(any(test, feature = "test-utils"))]
pub mod test_signing {
    use jsonwebtoken::{encode, Algorithm, DecodingKey, EncodingKey, Header};
    use serde_json::Value;
    use std::sync::OnceLock;

    pub const TEST_KID: &str = "phase-3c1-test";

    struct Material {
        encoding: EncodingKey,
        decoding: DecodingKey,
    }

    fn material() -> &'static Material {
        static KEYS: OnceLock<Material> = OnceLock::new();
        KEYS.get_or_init(|| {
            let key_pair =
                rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).expect("test keygen");
            let pem = key_pair.serialize_pem();
            let public_pem = key_pair.public_key_pem();
            Material {
                encoding: EncodingKey::from_ec_pem(pem.as_bytes()).expect("encoding key"),
                decoding: DecodingKey::from_ec_pem(public_pem.as_bytes()).expect("decoding key"),
            }
        })
    }

    pub fn verifier() -> DecodingKey {
        material().decoding.clone()
    }

    pub fn sign_token(claims: Value) -> String {
        let mut header = Header::new(Algorithm::ES256);
        header.kid = Some(TEST_KID.to_string());
        encode(&header, &claims, &material().encoding).expect("sign jwt")
    }

    pub fn user_token(sub: &str, issuer: &str, audience: &str, exp_offset_secs: i64) -> String {
        sign_token(serde_json::json!({
            "sub": sub,
            "iss": issuer,
            "aud": audience,
            "exp": chrono::Utc::now().timestamp() + exp_offset_secs,
        }))
    }
}
