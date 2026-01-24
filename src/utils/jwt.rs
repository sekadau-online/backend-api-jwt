use jsonwebtoken::{encode, decode, Header, EncodingKey, DecodingKey, Validation};
use serde::{Serialize, Deserialize};
use chrono::{Utc, Duration};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Claims {
    pub sub: i64, // user id
    pub exp: usize,
}

//function to create a JWT token
pub fn create_jwt(user_id: i64, secret: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Set expiration to 24 hours from now
    let expiration = Utc::now()
        .checked_add_signed(Duration::hours(24))
        .ok_or_else(|| Box::<dyn std::error::Error + Send + Sync>::from("failed to compute token expiration"))?
        .timestamp() as usize;

    // Create the claims and encode
    let token = encode(
        &Header::default(),
        &Claims {
            sub: user_id,
            exp: expiration,
        },
        &EncodingKey::from_secret(secret.as_ref()),
    ).map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    Ok(token)
}

//function to decode and validate a JWT token
pub fn decode_jwt(token: &str, secret: &str) -> Result<Claims, Box<dyn std::error::Error + Send + Sync>> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::default(),
    ).map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    // Return the claims if the token is valid
    Ok(token_data.claims)
}

// Async helper to generate a token using JWT_SECRET from environment
pub async fn generate_jwt_token(user_id: i64) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let secret = std::env::var("JWT_SECRET").map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    create_jwt(user_id, &secret)
}

// Async helper to verify a token using JWT_SECRET and return Claims
pub async fn verify_jwt_token(token: &str) -> Result<Claims, Box<dyn std::error::Error + Send + Sync>> {
    let secret = std::env::var("JWT_SECRET").map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    decode_jwt(token, &secret)
}