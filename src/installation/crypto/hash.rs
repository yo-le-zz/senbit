// installation/crypto/hash.rs

use anyhow::Result;
use argon2::{
    password_hash::PasswordHasher,
    Argon2,
};

pub fn hash_password(password: &str) -> Result<String> {
    let argon2 = Argon2::default();

    let password_hash = argon2
        .hash_password(password.as_bytes())?
        .to_string();

    Ok(password_hash)
}