use std::fmt;

use axum::{async_trait, extract::FromRequestParts, http::request::Parts};

use crate::n_number::ALLCHARS;

use super::AppError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AircraftSearch {
    ModeS(ModeS),
    Registration(Registration),
}

impl fmt::Display for AircraftSearch {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::ModeS(x) => write!(f, "{}", x.0),
            Self::Registration(x) => write!(f, "{}", x.0),
        }
    }
}

trait Validate {
    fn validate(x: &str) -> Result<String, AppError>;
}

/// Check that a given char is 0-9, a-END
fn valid_char(c: char, end: char) -> bool {
    c.is_ascii_digit() || ('a'..=end).contains(&c.to_ascii_lowercase())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registration(String);

impl Registration {
    /// Check if the all chars in a given &str are valid
    pub fn is_valid(input: &str) -> bool {
        input.chars().all(|c| valid_char(c, 'z') || c == '-')
    }
}

impl fmt::Display for Registration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<&str> for Registration {
    type Error = AppError;
    fn try_from(x: &str) -> Result<Self, AppError> {
        Ok(Self(Self::validate(x)?))
    }
}

impl TryFrom<&String> for Registration {
    type Error = AppError;
    fn try_from(x: &String) -> Result<Self, AppError> {
        Ok(Self(Self::validate(x)?))
    }
}

impl TryFrom<String> for Registration {
    type Error = AppError;
    fn try_from(x: String) -> Result<Self, AppError> {
        Ok(Self(Self::validate(&x)?))
    }
}

impl Validate for Registration {
    /// Make sure that input is a valid registration, less than 16 chars, and convert to uppercase
    fn validate(input: &str) -> Result<String, AppError> {
        if Self::is_valid(input) && input.len() <= 16 && !input.is_empty() {
            Ok(input.to_uppercase())
        } else {
            Err(AppError::Registration(input.to_owned()))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeS(String);

impl ModeS {
    /// Check that all chars in a given &str are valid
    pub fn is_valid(input: &str) -> bool {
        input.chars().all(|c| valid_char(c, 'f'))
    }
}
impl fmt::Display for ModeS {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<&str> for ModeS {
    type Error = AppError;
    fn try_from(x: &str) -> Result<Self, AppError> {
        Ok(Self(Self::validate(x)?))
    }
}

impl TryFrom<&String> for ModeS {
    type Error = AppError;
    fn try_from(x: &String) -> Result<Self, AppError> {
        Ok(Self(Self::validate(x)?))
    }
}

impl TryFrom<String> for ModeS {
    type Error = AppError;
    fn try_from(x: String) -> Result<Self, AppError> {
        Ok(Self(Self::validate(&x)?))
    }
}

impl Validate for ModeS {
    /// Make sure that input is an uppercase valid mode_s string, validity is [a-f]{6}
    fn validate(input: &str) -> Result<String, AppError> {
        if input.len() == 6 && Self::is_valid(input) {
            Ok(input.to_uppercase())
        } else {
            Err(AppError::ModeS(input.to_owned()))
        }
    }
}

// This should be Aircraft search
#[async_trait]
impl<S> FromRequestParts<S> for AircraftSearch
where
    S: Send + Sync,
{
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match axum::extract::Path::<String>::from_request_parts(parts, state).await {
            Ok(value) => {
                if let Ok(mode_s) = ModeS::try_from(&value.0) {
                    return Ok(Self::ModeS(mode_s));
                }
                if let Ok(registration) = Registration::try_from(&value.0) {
                    return Ok(Self::Registration(registration));
                }
                Err(AppError::AircraftSearch(value.0))
            }
            Err(_) => Err(AppError::AircraftSearch(String::new())),
        }
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for ModeS
where
    S: Send + Sync,
{
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match axum::extract::Path::<String>::from_request_parts(parts, state).await {
            Ok(value) => Ok(Self::try_from(value.0)?),
            Err(_) => Err(AppError::AircraftSearch(String::new())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NNumber(String);

impl fmt::Display for NNumber {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<&str> for NNumber {
    type Error = AppError;
    fn try_from(x: &str) -> Result<Self, AppError> {
        Ok(Self(Self::validate(x)?))
    }
}

impl TryFrom<String> for NNumber {
    type Error = AppError;
    fn try_from(x: String) -> Result<Self, AppError> {
        Ok(Self(Self::validate(&x)?))
    }
}

impl Validate for NNumber {
    /// Make sure that input is an uppercase valid n_number string, validity is N[0-9 a-z (but not I or O)]{1-5}
    fn validate(input: &str) -> Result<String, AppError> {
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
impl<S> FromRequestParts<S> for NNumber
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match axum::extract::Path::<String>::from_request_parts(parts, state).await {
            Ok(value) => Ok(Self::try_from(value.0)?),
            Err(_) => Err(AppError::NNumber(String::from("invalid"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Callsign(String);

impl TryFrom<&str> for Callsign {
    type Error = AppError;
    fn try_from(x: &str) -> Result<Self, AppError> {
        Ok(Self(Self::validate(x)?))
    }
}

impl TryFrom<String> for Callsign {
    type Error = AppError;
    fn try_from(x: String) -> Result<Self, AppError> {
        Ok(Self(Self::validate(&x)?))
    }
}

impl TryFrom<&String> for Callsign {
    type Error = AppError;
    fn try_from(x: &String) -> Result<Self, AppError> {
        Ok(Self(Self::validate(x)?))
    }
}

impl fmt::Display for Callsign {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Validate for Callsign {
    /// Make sure that input is a uppercaser valid callsign String, validity is [a-z]{4-8}
    fn validate(input: &str) -> Result<String, AppError> {
        if (4..=8).contains(&input.len()) && input.chars().all(|c| valid_char(c, 'z')) {
            Ok(input.to_uppercase())
        } else {
            Err(AppError::Callsign(input.to_owned()))
        }
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for Callsign
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match axum::extract::Path::<String>::from_request_parts(parts, state).await {
            Ok(value) => Ok(Self::try_from(value.0)?),
            Err(_) => Err(AppError::AircraftSearch(String::from("invalid"))),
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
        let result = valid_char(char, 'z');
        assert!(result);

        let char = '1';
        let result = valid_char(char, 'b');
        assert!(result);
    }

    #[test]
    fn mod_api_input_charset_invalid() {
        let char = 'g';
        let result = valid_char(char, 'b');
        assert!(!result);

        let char = '%';
        let result = valid_char(char, 'b');
        assert!(!result);
    }

    #[test]
    fn mod_api_input_callsign_ok() {
        let test = |input: &str| {
            let result = Callsign::try_from(input);
            assert!(result.is_ok());
            assert_eq!(result.unwrap().0, input.to_uppercase());
        };

        test("AaBb12");
        test("AaaA1111");
    }

    #[test]
    fn mod_api_input_callsign_err() {
        let test = |input: &str| {
            let result = Callsign::try_from(input);
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
            let result = NNumber::try_from(input);
            assert!(result.is_ok());
            assert_eq!(result.unwrap().0, input.to_uppercase());
        };

        test("N2");
        test("N124ZA");
        test("Nff45");
    }

    #[test]
    fn mod_api_input_n_number_err() {
        let test = |input: &str| {
            let result = NNumber::try_from(input);
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
        // Doesn't start with N
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
            let result = ModeS::try_from(input);
            assert!(result.is_ok());
            assert_eq!(result.unwrap().0, input.to_uppercase());
        };

        test("AaBb12");
        test("C03BF1");
    }

    #[test]
    fn mod_api_input_mode_s_err() {
        let test = |input: &str| {
            let result = ModeS::try_from(input);
            assert!(result.is_err());
            match result.unwrap_err() {
                AppError::ModeS(err) => assert_eq!(err, input),
                _ => unreachable!(),
            };
        };

        // Empty
        test("");
        // Too short
        test("aaaaa");
        // Too long
        test("bbbbbbb");
        // contains invalid alpha char
        test("aaa12h");
        // contains invalid non-alpha char
        test("aaa12$");
    }

    #[test]
    fn mod_api_input_registration_ok() {
        let test = |input: &str| {
            let result = Registration::try_from(input);
            assert!(result.is_ok());
            assert_eq!(result.unwrap().0, input.to_uppercase());
        };

        test("n1245");
        test("b-818-f");
    }

    #[test]
    fn mod_api_input_registration_err() {
        let test = |input: &str| {
            let result = Registration::try_from(input);
            match result.unwrap_err() {
                AppError::Registration(err) => assert_eq!(err, input),
                _ => unreachable!(),
            };
        };

        // Empty
        test("");
        // Too long
        test("ababababababababab");
        // contains an invalid char
        test("abhyuio$pa");
    }
}
