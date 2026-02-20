use base64::{Engine as _, engine::general_purpose::STANDARD};
use pbkdf2::pbkdf2_hmac;
use rand::{Rng, distributions::Alphanumeric};
use sha2::Sha256;

const DEFAULT_ITERATIONS: u32 = 600_000;

pub fn hash_password(raw_password: &str) -> String {
    let iterations = std::env::var("DJANGO_PBKDF2_ITERATIONS")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(DEFAULT_ITERATIONS);

    let salt: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(12)
        .map(char::from)
        .collect();

    let mut out = [0u8; 32];
    pbkdf2_hmac::<Sha256>(
        raw_password.as_bytes(),
        salt.as_bytes(),
        iterations,
        &mut out,
    );
    let encoded = STANDARD.encode(out);

    format!("pbkdf2_sha256${iterations}${salt}${encoded}")
}

pub fn verify_password(raw_password: &str, encoded: &str) -> bool {
    // Django 默认格式：algorithm$iterations$salt$hash
    let mut parts = encoded.split('$');
    let Some(algorithm) = parts.next() else {
        return false;
    };
    let Some(iterations_str) = parts.next() else {
        return false;
    };
    let Some(salt) = parts.next() else {
        return false;
    };
    let Some(hash_b64) = parts.next() else {
        return false;
    };

    if algorithm != "pbkdf2_sha256" {
        return false;
    }
    let Ok(iterations) = iterations_str.parse::<u32>() else {
        return false;
    };

    let Ok(expected) = STANDARD.decode(hash_b64.as_bytes()) else {
        return false;
    };
    let mut out = vec![0u8; expected.len()];
    pbkdf2_hmac::<Sha256>(
        raw_password.as_bytes(),
        salt.as_bytes(),
        iterations,
        &mut out,
    );

    // 常量时间比较不是强制，但这里避免明显 side-channel
    constant_time_eq(&out, &expected)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}
