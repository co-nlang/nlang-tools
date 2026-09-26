use crate::value::{AuthorityInfo, ContentHash, Identity};
use ring::signature::{self, UnparsedPublicKey};
use std::collections::HashSet;

pub enum AuthVerifyResult {
    Valid,
    Exempt,
    Invalid(String),
}

pub fn compute_refine_payload(
    source_caids: &[ContentHash],
    target_caids: &[ContentHash],
) -> Vec<u8> {
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
    })
}

/// Ed25519 over `refine-commit:v1:` plus the CAID of the commit value with
/// `refine.authority` absent. The CAID is the string `content_hash` prints.
pub fn sign_commit(caid: &str, identity: &Identity) -> Result<AuthorityInfo, String> {
    let payload = format!("refine-commit:v1:{caid}");
    sign_refine(payload.as_bytes(), identity)
}

/// What a stored signature actually covers. Membership is not consulted.
pub enum SignatureCoverage {
    /// `refine-commit:v1:` plus the CAID of the commit without `authority`.
    Commit,
    /// `refine:` plus the sorted source and target CAIDs.
    SourcesAndTargets,
}

fn signature_bytes_hold(auth: &AuthorityInfo, payload: &[u8]) -> bool {
    check_commit_signature(auth, payload).is_ok()
}

/// Cryptographic check only. The error strings match `verify_refine_authority`.
pub fn check_commit_signature(auth: &AuthorityInfo, payload: &[u8]) -> Result<(), String> {
    let pk = hex::decode(&auth.signer_pubkey_hex)
        .map_err(|e| format!("bad pubkey hex: {}", e))?;
    let sig = hex::decode(&auth.signature_hex)
        .map_err(|e| format!("bad signature hex: {}", e))?;
    UnparsedPublicKey::new(&signature::ED25519, pk)
        .verify(payload, &sig)
        .map_err(|_| "Ed25519 signature verification failed".to_string())
}

/// The public key, and which payload it signed. `None` when there is no
/// signature or neither payload verifies.
pub fn signature_coverage(commit: &crate::value::Commit) -> Option<(&str, SignatureCoverage)> {
    let ri = commit.refine_info.as_ref()?;
    let auth = ri.authority.as_ref()?;
    let mut unsigned = commit.clone();
    if let Some(info) = unsigned.refine_info.as_mut() {
        info.authority = None;
    }
    let body = crate::store_codec::commit_body(&unsigned);
    if let Ok(caid) = crate::store_codec::commit_body_address(&body) {
        let payload = format!("refine-commit:v1:{caid}");
        if signature_bytes_hold(auth, payload.as_bytes()) {
            return Some((auth.signer_pubkey_hex.as_str(), SignatureCoverage::Commit));
        }
    }
    let old = compute_refine_payload(&ri.source_caids, &ri.target_caids);
    if signature_bytes_hold(auth, &old) {
        return Some((
            auth.signer_pubkey_hex.as_str(),
            SignatureCoverage::SourcesAndTargets,
        ));
    }
    None
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
        Ok(b) => b,
        Err(e) => return AuthVerifyResult::Invalid(format!("bad pubkey hex: {}", e)),
    };
    let sig_bytes = match hex::decode(&auth.signature_hex) {
        Ok(b) => b,
        Err(e) => return AuthVerifyResult::Invalid(format!("bad signature hex: {}", e)),
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
