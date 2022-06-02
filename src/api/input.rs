use async_trait::async_trait;
use axum::extract::{FromRequest, RequestParts};

use super::AppError;

// Check if input char is 0-9, a-end
fn is_charset(c: char, end: char) -> bool {
    c.is_ascii_digit() || ('a'..=end).contains(&c.to_ascii_lowercase())
}

pub fn is_hex(input: &str) -> bool {
    input.chars().all(|c| is_charset(c, 'f'))
}

trait Validate {
    fn validate(x: String) -> Result<String, AppError>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModeS {
    pub mode_s: String,
}

impl ModeS {
    pub fn new(x: String) -> Result<Self, AppError> {
        Ok(Self {
            mode_s: Self::validate(x)?,
        })
    }
}
impl Validate for ModeS {
    /// Make sure that input is an uppercase valid mode_s string, validitiy is [a-f]{6}
    fn validate(input: String) -> Result<String, AppError> {
        let valid = input.len() == 6 && is_hex(&input);
        if valid {
            Ok(input.to_uppercase())
        } else {
            Err(AppError::ModeS(input))
        }
    }
}

#[async_trait]
impl<B> FromRequest<B> for ModeS
where
    B: Send,
{
    type Rejection = AppError;
    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        match axum::extract::Path::<String>::from_request(req).await {
            Ok(value) => Ok(ModeS::new(value.0)?),
            Err(_) => Err(AppError::ModeS(String::from("invalid"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Callsign {
    pub callsign: String,
}

impl Callsign {
    pub fn new(x: String) -> Result<Self, AppError> {
        Ok(Self {
            callsign: Self::validate(x)?,
        })
    }
}

impl Validate for Callsign {
    // Make sure that input is a uppercaser valid callsign String, validitiy is [a-z]{4-8}
    fn validate(input: String) -> Result<String, AppError> {
        let valid = (4..=8).contains(&input.len()) && input.chars().all(|c| is_charset(c, 'z'));
        if valid {
            Ok(input.to_uppercase())
        } else {
            Err(AppError::Callsign(input))
        }
    }
}

#[async_trait]
impl<B> FromRequest<B> for Callsign
where
    B: Send,
{
    type Rejection = AppError;
    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        match axum::extract::Path::<String>::from_request(req).await {
            Ok(value) => Ok(Callsign::new(value.0)?),
            Err(_) => Err(AppError::ModeS(String::from("invalid"))),
        }
    }
}

/// ApiRoutes tests
/// cargo watch -q -c -w src/ -x 'test mod_api_input -- --test-threads=1 --nocapture'
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mod_api_input_charset_valid() {
        let char = 'a';
        let result = is_charset(char, 'z');
        assert!(result);

        let char = '1';
        let result = is_charset(char, 'b');
        assert!(result);
    }

    #[test]
    fn mod_api_input_charset_invalid() {
        let char = 'g';
        let result = is_charset(char, 'b');
        assert!(!result);

        let char = '%';
        let result = is_charset(char, 'b');
        assert!(!result);
    }

    #[test]
    fn mod_api_input_callsign_ok() {
        let test = |input: &str| {
            let result = Callsign::new(input.to_owned());
            assert!(result.is_ok());
            assert_eq!(result.unwrap().callsign, input.to_uppercase());
        };

        test("AaBb12");
        test("AaaA1111");
    }

    #[test]
    fn mod_api_input_callsign_err() {
        let test = |input: &str| {
            let result = Callsign::new(input.to_owned());
            assert!(result.is_err());
            match result.unwrap_err() {
                AppError::Callsign(err) => assert_eq!(err, input),
                _ => unreachable!(),
            };
        };

        // Too short
        test("aaa");
        // Too long
        test("bbbbbbbbb");
        // contains invalid char
        test("aaa124*");
    }

    #[test]
    fn mod_api_input_mode_s_of() {
        let test = |input: &str| {
            let result = ModeS::new(input.to_owned());
            assert!(result.is_ok());
            assert_eq!(result.unwrap().mode_s, input.to_uppercase());
        };

        test("AaBb12");
        test("C03BF1");
    }

    #[test]
    fn mod_api_input_mode_s_err() {
        let test = |input: &str| {
            let result = ModeS::new(input.to_owned());
            assert!(result.is_err());
            match result.unwrap_err() {
                AppError::ModeS(err) => assert_eq!(err, input),
                _ => unreachable!(),
            };
        };

        // Too short
        test("aaaaa");
        // Too long
        test("bbbbbbb");
        // contains invalid alpha char
        test("aaa12h");
        // contains invalid non-alpha char
        test("aaa12$");
    }
}
