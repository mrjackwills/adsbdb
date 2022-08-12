use std::fmt;

use async_trait::async_trait;
use axum::extract::{FromRequest, RequestParts};

use crate::n_number::ALLCHARS;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeS {
    mode_s: String,
}

impl fmt::Display for ModeS {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.mode_s)
    }
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
            Ok(value) => Ok(Self::new(value.0)?),
            Err(_) => Err(AppError::ModeS(String::from("invalid"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NNumber {
    n_number: String,
}

impl fmt::Display for NNumber {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.n_number)
    }
}

impl NNumber {
    pub fn new(x: String) -> Result<Self, AppError> {
        Ok(Self {
            n_number: Self::validate(x)?,
        })
    }
}
impl Validate for NNumber {
    /// Make sure that input is an uppercase valid n_number string, validitiy is N[0-9 a-z (but not I or O)]{1-5}
    fn validate(input: String) -> Result<String, AppError> {
        let input = input.to_uppercase();
        if input.starts_with('N')
            && (2..=6).contains(&input.chars().count())
            && input.chars().all(|x| ALLCHARS.contains(x))
        {
            Ok(input.to_uppercase())
        } else {
            Err(AppError::NNumber(input))
        }
    }
}

#[async_trait]
impl<B> FromRequest<B> for NNumber
where
    B: Send,
{
    type Rejection = AppError;
    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        match axum::extract::Path::<String>::from_request(req).await {
            Ok(value) => Ok(Self::new(value.0)?),
            Err(_) => Err(AppError::NNumber(String::from("invalid"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
            Ok(value) => Ok(Self::new(value.0)?),
            Err(_) => Err(AppError::ModeS(String::from("invalid"))),
        }
    }
}

/// cargo watch -q -c -w src/ -x 'test mod_api_input -- --nocapture'
#[cfg(test)]
#[allow(clippy::pedantic, clippy::nursery, clippy::unwrap_used)]
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
    fn mod_api_input_n_number_ok() {
        let test = |input: &str| {
            let result = NNumber::new(input.to_owned());
            assert!(result.is_ok());
            assert_eq!(result.unwrap().n_number, input.to_uppercase());
        };

        test("N2");
        test("N124ZA");
        test("Nff45");
    }

    #[test]
    fn mod_api_input_n_number_err() {
        let test = |input: &str| {
            let result = NNumber::new(input.to_owned());
            assert!(result.is_err());
            match result.unwrap_err() {
                AppError::NNumber(err) => assert_eq!(err, input.to_uppercase()),
                _ => unreachable!(),
            };
        };

        // Too short
        test("N");
        // Too long
        test("Naaaaaa");
        // Doens't start with N
        test("Aaaaaaa");
        // contains invalid  char
        test("n1234o");
        // contains invalid  char
        test("n1234i");
        // contains invalid non-alpha char
        test("Naa12$");
    }

    #[test]
    fn mod_api_input_mode_s_ok() {
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
