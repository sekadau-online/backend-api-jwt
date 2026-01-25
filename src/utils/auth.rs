use tokio::time::{Duration, timeout};

/// Verify a password against a bcrypt hash in a blocking thread with a timeout.
/// Returns Ok(true) if password matches, Ok(false) if not, Err on internal errors/timeouts.
pub async fn verify_password_blocking(
    password: String,
    hashed: String,
    timeout_secs: Option<u64>,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let task = tokio::task::spawn_blocking(move || bcrypt::verify(&password, &hashed));
    let dur = Duration::from_secs(timeout_secs.unwrap_or(5));

    match timeout(dur, task).await {
        Ok(join_res) => match join_res {
            Ok(Ok(valid)) => Ok(valid),
            Ok(Err(e)) => Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
            Err(join_err) => Err(Box::new(join_err) as Box<dyn std::error::Error + Send + Sync>),
        },
        Err(_) => Err(Box::<dyn std::error::Error + Send + Sync>::from(
            "password verification timed out",
        )),
    }
}

/// Hash a password in a blocking thread with a timeout and configurable cost.
pub async fn hash_password_blocking(
    password: String,
    cost: u32,
    timeout_secs: Option<u64>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let task = tokio::task::spawn_blocking(move || bcrypt::hash(&password, cost));
    let dur = Duration::from_secs(timeout_secs.unwrap_or(5));

    match timeout(dur, task).await {
        Ok(join_res) => match join_res {
            Ok(Ok(hash)) => Ok(hash),
            Ok(Err(e)) => Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
            Err(join_err) => Err(Box::new(join_err) as Box<dyn std::error::Error + Send + Sync>),
        },
        Err(_) => Err(Box::<dyn std::error::Error + Send + Sync>::from(
            "password hashing timed out",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hash_and_verify() {
        let pw = "hunter2".to_string();
        let hash = hash_password_blocking(pw.clone(), bcrypt::DEFAULT_COST, Some(5))
            .await
            .expect("hash");
        let ok = verify_password_blocking(pw, hash, Some(5))
            .await
            .expect("verify");
        assert!(ok);
    }
}
