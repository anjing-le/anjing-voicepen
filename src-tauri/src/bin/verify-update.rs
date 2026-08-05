use std::{env, fs, process};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use minisign_verify::{PublicKey, Signature};

fn verify() -> Result<(), ()> {
    let mut arguments = env::args_os().skip(1);
    let public_key = arguments.next().ok_or(())?;
    let signature = arguments.next().ok_or(())?;
    let asset_path = arguments.next().ok_or(())?;
    if arguments.next().is_some() {
        return Err(());
    }

    let public_key = public_key.into_string().map_err(|_| ())?;
    let signature = signature.into_string().map_err(|_| ())?;
    let public_key = STANDARD.decode(public_key.trim()).map_err(|_| ())?;
    let signature = STANDARD.decode(signature.trim()).map_err(|_| ())?;
    let public_key = std::str::from_utf8(&public_key).map_err(|_| ())?;
    let signature = std::str::from_utf8(&signature).map_err(|_| ())?;
    let public_key = PublicKey::decode(public_key).map_err(|_| ())?;
    let signature = Signature::decode(signature).map_err(|_| ())?;
    let asset = fs::read(asset_path).map_err(|_| ())?;

    public_key.verify(&asset, &signature, true).map_err(|_| ())
}

fn main() {
    if verify().is_ok() {
        println!("update signature valid");
    } else {
        eprintln!("update signature verification failed");
        process::exit(1);
    }
}
