//! PKCE (RFC 7636) helpers for the OAuth authorization code flow.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use rand::RngCore;
use sha2::{Digest, Sha256};

/// A PKCE code verifier paired with its challenge.
#[derive(Clone, Debug)]
pub struct PkcePair {
    pub verifier: String,
    pub challenge: String,
}

impl PkcePair {
    /// Generate a fresh PKCE pair using S256.
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        let verifier = URL_SAFE_NO_PAD.encode(bytes);
        let challenge = challenge_from_verifier(&verifier);
        Self {
            verifier,
            challenge,
        }
    }
}

/// Generate a random URL-safe state token.
pub fn generate_state() -> String {
    let mut bytes = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn challenge_from_verifier(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifier_is_base64url_no_pad_43_chars() {
        let pair = PkcePair::generate();
        assert_eq!(pair.verifier.len(), 43, "32 bytes -> 43 base64url chars");
        assert!(!pair.verifier.contains('='));
        assert!(!pair.verifier.contains('+'));
        assert!(!pair.verifier.contains('/'));
    }

    #[test]
    fn challenge_is_deterministic_from_verifier() {
        let v = "test_verifier_value_should_hash_to_known_output";
        assert_eq!(challenge_from_verifier(v), challenge_from_verifier(v));
    }

    #[test]
    fn challenge_matches_rfc_7636_appendix_b_example() {
        // RFC 7636 Appendix B reference vector.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let expected = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        assert_eq!(challenge_from_verifier(verifier), expected);
    }

    #[test]
    fn state_tokens_are_unique() {
        let a = generate_state();
        let b = generate_state();
        assert_ne!(a, b);
    }
}
