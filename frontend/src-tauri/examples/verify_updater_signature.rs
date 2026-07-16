use base64::{engine::general_purpose::STANDARD, Engine as _};
use minisign_verify::{PublicKey, Signature};
use serde_json::Value;
use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let artifact = PathBuf::from(args.next().ok_or("missing artifact path")?);
    let signature_file = PathBuf::from(args.next().ok_or("missing signature path")?);
    let tauri_config = PathBuf::from(args.next().ok_or("missing tauri config path")?);
    if args.next().is_some() {
        return Err("usage: verify_updater_signature <artifact> <signature> <tauri-config>".into());
    }

    let config: Value = serde_json::from_slice(&fs::read(tauri_config)?)?;
    let encoded_public_key = config
        .pointer("/plugins/updater/pubkey")
        .and_then(Value::as_str)
        .ok_or("tauri config does not contain plugins.updater.pubkey")?;
    let public_key_text = String::from_utf8(STANDARD.decode(encoded_public_key.trim())?)?;
    let public_key = PublicKey::decode(&public_key_text)?;

    let encoded_signature = fs::read_to_string(signature_file)?;
    let signature_text = String::from_utf8(STANDARD.decode(encoded_signature.trim())?)?;
    let signature = Signature::decode(&signature_text)?;

    let artifact_bytes = fs::read(&artifact)?;
    public_key.verify(&artifact_bytes, &signature, true)?;
    println!("Verified updater signature for {}", artifact.display());
    Ok(())
}
