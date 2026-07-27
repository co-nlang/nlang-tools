use crate::value::{AuthorityInfo, ContentHash, Identity};
use ring::signature::{self, UnparsedPublicKey};
use std::collections::HashSet;

pub enum AuthVerifyResult {
    Valid,
    Exempt,
    Invalid(String),
}

pub fn compute_refine_payload(source_caids: &[ContentHash], target_caids: &[ContentHash]) -> Vec<u8> {
    let mut srcs: Vec<String> = source_caids.iter().map(|c| c.to_string()).collect();
    let mut tgts: Vec<String> = target_caids.iter().map(|c| c.to_string()).collect();
    srcs.sort();
    tgts.sort();
    format!("refine:{}:{}", srcs.join("|"), tgts.join("|")).into_bytes()
}

pub fn sign_refine(payload: &[u8], identity: &Identity) -> Result<AuthorityInfo, String> {
    let key_pair = signature::Ed25519KeyPair::from_pkcs8(&identity.private_key)
        .map_err(|e| format!("invalid private key: {:?}", e))?;
    let sig = key_pair.sign(payload);
    Ok(AuthorityInfo {
        signer_pubkey_hex: hex::encode(&identity.public_key),
        signature_hex: hex::encode(sig.as_ref()),
        timestamp: Some(chrono::Utc::now().to_rfc3339()),
    })
}

pub fn verify_refine_authority(
    authority: Option<&AuthorityInfo>,
    payload: &[u8],
    architect_registry: &HashSet<String>,
    bootstrap_exempt: bool,
) -> AuthVerifyResult {
    let auth = match authority {
        None => {
            return if bootstrap_exempt {
                AuthVerifyResult::Exempt
            } else {
                AuthVerifyResult::Invalid("missing %authority on non-bootstrap refine".to_string())
            };
        }
        Some(a) => a,
    };

    let pk_bytes = match hex::decode(&auth.signer_pubkey_hex) {
        Ok(b) => b, Err(e) => return AuthVerifyResult::Invalid(format!("bad pubkey hex: {}", e)),
    };
    let sig_bytes = match hex::decode(&auth.signature_hex) {
        Ok(b) => b, Err(e) => return AuthVerifyResult::Invalid(format!("bad signature hex: {}", e)),
    };

    // Membership: a non-empty whitelist that does not contain the signer is
    // always a refusal. Under bootstrap exemption with an *empty* registry
    // (universe_determinism — no self-appointment), there is no set to be a
    // member of; skip membership and crypto-check only. Production never has
    // empty registry without bootstrap_exempt (`empty ⇒ exempt` in refine).
    let skip_membership = bootstrap_exempt && architect_registry.is_empty();
    if !skip_membership && !architect_registry.contains(&auth.signer_pubkey_hex) {
        return AuthVerifyResult::Invalid(format!(
            "signer {} not in architect_registry",
            &auth.signer_pubkey_hex
        ));
    }

    let vk = UnparsedPublicKey::new(&signature::ED25519, &pk_bytes);
    match vk.verify(payload, &sig_bytes) {
        Ok(()) => {
            // Crypto ok. Record "verified" only when a whitelist constrained
            // membership; bootstrap-with-empty-registry remains "unverified"
            // (no authority existed to verify against).
            if skip_membership {
                AuthVerifyResult::Exempt
            } else {
                AuthVerifyResult::Valid
            }
        }
        Err(_) => AuthVerifyResult::Invalid("Ed25519 signature verification failed".to_string()),
    }
}
