use crate::{SignerVerification, VerifyInput};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

pub fn sign(seed: &[u8; 32], message: &[u8]) -> SignerVerification {
    let sk = SigningKey::from_bytes(seed);
    let pubkey = sk.verifying_key().to_bytes().to_vec();
    let signature = sk.sign(message).to_bytes().to_vec();
    SignerVerification { pubkey, signature }
}

pub fn verify_all(input: &VerifyInput) -> Result<(), String> {
    for s in &input.signers {
        let pk_arr: [u8; 32] = s
            .pubkey
            .as_slice()
            .try_into()
            .map_err(|_| "pubkey: expected 32 bytes".to_string())?;
        let vk = VerifyingKey::from_bytes(&pk_arr).map_err(|e| format!("pubkey: {e}"))?;
        let sig = Signature::from_slice(&s.signature).map_err(|e| format!("sig: {e}"))?;
        vk.verify_strict(&input.message, &sig)
            .map_err(|e| format!("verify: {e}"))?;
    }
    Ok(())
}
