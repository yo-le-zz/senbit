use anyhow::Result;

use argon2::{
    Argon2,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};

use crypt_sha512::{Password, hash};

#[allow(dead_code)] // kept as public API of this module
pub fn hash_password(password: &str) -> Result<String> {
    let password_hash = Argon2::default()
        .hash_password(password.as_bytes())?
        .to_string();

    Ok(password_hash)
}

#[allow(dead_code)] // kept as public API of this module
pub fn verify_password(password: &str, password_hash: &str) -> Result<bool> {
    let parsed_hash = PasswordHash::new(password_hash)?;

    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

pub fn hash_shadow_password(password: &str) -> String {
    hash(Password::from(password.to_string()), None)
}

