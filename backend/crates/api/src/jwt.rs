use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, TokenData, Validation};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct JwtService {
    encoding: EncodingKey,
    decoding: DecodingKey,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub token_type: String,
}

impl JwtService {
    pub fn from_secret(secret: &str) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret.as_bytes()),
            decoding: DecodingKey::from_secret(secret.as_bytes()),
        }
    }

    pub fn issue_access_token(&self, user_id: &str) -> String {
        let exp = (Utc::now() + Duration::hours(1)).timestamp() as usize;
        let claims = Claims {
            sub: user_id.to_string(),
            exp,
            token_type: "access".to_string(),
        };
        jsonwebtoken::encode(&Header::default(), &claims, &self.encoding).expect("encode access")
    }

    pub fn issue_refresh_token(&self, user_id: &str) -> String {
        let exp = (Utc::now() + Duration::days(7)).timestamp() as usize;
        let claims = Claims {
            sub: user_id.to_string(),
            exp,
            token_type: "refresh".to_string(),
        };
        jsonwebtoken::encode(&Header::default(), &claims, &self.encoding).expect("encode refresh")
    }

    pub fn decode(&self, token: &str) -> Result<TokenData<Claims>, jsonwebtoken::errors::Error> {
        let mut validation = Validation::default();
        validation.validate_exp = true;
        jsonwebtoken::decode::<Claims>(token, &self.decoding, &validation)
    }
}
