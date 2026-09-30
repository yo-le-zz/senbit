use anyhow::Result;
use argon2::{
    password_hash::{
        PasswordHasher,
        PasswordVerifier,
        phc::PasswordHash,
    },
    Argon2,
};

pub fn hash_password(password: &str) -> Result<String> {
    let password_hash = Argon2::default()
        .hash_password(password.as_bytes())?
        .to_string();

    Ok(password_hash)
}

pub fn verify_password(password: &str, password_hash: &str) -> Result<bool> {
    let parsed_hash = PasswordHash::new(password_hash)?;

    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}