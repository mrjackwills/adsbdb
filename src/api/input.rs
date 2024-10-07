use std::fmt;

use axum::{async_trait, extract::FromRequestParts, http::request::Parts};

use crate::n_number::{n_number_to_mode_s, ALLCHARS};

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

/// This should take impl to string and result as Self
pub trait Validate {
    fn validate(x: &str) -> Result<Self, AppError>
    where
        Self: Sized;
}

/// Check that a given char is 0-9, a-END, will lowercase everything
fn valid_char(c: char, end: char) -> bool {
    c.is_ascii_digit() || ('a'..=end).contains(&c.to_ascii_lowercase())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AirlineCode {
    Iata(String),
    Icao(String),
}

impl fmt::Display for AirlineCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Iata(x) | Self::Icao(x) => write!(f, "{x}"),
        }
    }
}

impl Validate for AirlineCode {
    /// Make sure that input is a valid airline short code, [a-z]{3-4}, and return as uppercase
    fn validate(input: &str) -> Result<Self, AppError> {
        let input = input.to_uppercase();
        let count = input.chars().count();
        if !input.is_empty()
            && (2..=3).contains(&count)
            && input.chars().all(|c| valid_char(c, 'z'))
        {
            if count == 2 {
                Ok(Self::Iata(input))
            } else {
                Ok(Self::Icao(input))
            }
        } else {
            Err(AppError::Airline(input))
        }
    }
}

/// Make unit structs, StructName(String), and impl display on it
macro_rules! unit_struct {
    ($struct_name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $struct_name(String);

        impl std::fmt::Display for $struct_name {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

unit_struct!(ModeS);
unit_struct!(NNumber);
unit_struct!(Registration);

#[async_trait]
impl<S> FromRequestParts<S> for AircraftSearch
where
    S: Send + Sync,
{
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match axum::extract::Path::<String>::from_request_parts(parts, state).await {
            Ok(value) => {
                if let Ok(mode_s) = ModeS::validate(&value.0) {
                    return Ok(Self::ModeS(mode_s));
                }
                if let Ok(registration) = Registration::validate(&value.0) {
                    return Ok(Self::Registration(registration));
                }
                Err(AppError::AircraftSearch(value.0))
            }
            Err(_) => Err(AppError::AircraftSearch(String::new())),
        }
    }
}

/// from_request_parts macro, to run Self::validate
macro_rules! from_request_parts {
    ($struct_name:ident, AppError::$variant:ident) => {
        #[async_trait]
        impl<S> FromRequestParts<S> for $struct_name
        where
            S: Send + Sync,
        {
            type Rejection = AppError;
            async fn from_request_parts(
                parts: &mut Parts,
                state: &S,
            ) -> Result<Self, Self::Rejection> {
                match axum::extract::Path::<String>::from_request_parts(parts, state).await {
                    Ok(value) => Ok(Self::validate(&value.0)?),
                    Err(_) => Err(AppError::$variant(String::from("invalid"))),
                }
            }
        }
    };

    ($struct_name:ident) => {
        #[async_trait]
        impl<S> FromRequestParts<S> for $struct_name
        where
            S: Send + Sync,
        {
            type Rejection = AppError;
            async fn from_request_parts(
                parts: &mut Parts,
                state: &S,
            ) -> Result<Self, Self::Rejection> {
                match axum::extract::Path::<String>::from_request_parts(parts, state).await {
                    Ok(value) => Ok(Self::validate(&value.0)?),
                    Err(_) => Err(AppError::AircraftSearch(String::new())),
                }
            }
        }
    };
}

from_request_parts!(ModeS);
from_request_parts!(NNumber, AppError::NNumber);
from_request_parts!(Callsign, AppError::AircraftSearch);
from_request_parts!(AirlineCode);

impl Validate for Registration {
    /// Make sure that input is a valid registration, less than 16 chars, and convert to uppercase
    fn validate(input: &str) -> Result<Self, AppError> {
        let input = input.to_uppercase();
        if !input.is_empty()
            && input.len() <= 16
            && input.chars().all(|c| valid_char(c, 'z') || c == '-')
        {
            Ok(Self(input))
        } else {
            Err(AppError::Registration(input))
        }
    }
}

impl Validate for ModeS {
    /// Make sure that input is an uppercase valid mode_s string, validity is [a-f]{6}
    fn validate(input: &str) -> Result<Self, AppError> {
        let input = input.to_uppercase();
        if input.len() == 6 && input.chars().all(|c| valid_char(c, 'f')) {
            Ok(Self(input))
        } else {
            Err(AppError::ModeS(input))
        }
    }
}

impl Validate for NNumber {
    /// Make sure that input is an uppercase valid n_number string, validity is N[0-9 a-z (but not I or O)]{1-5}
    fn validate(input: &str) -> Result<Self, AppError> {
        let input = input.to_uppercase();
        if input.starts_with('N')
            && (2..=6).contains(&input.chars().count())
            && input.chars().all(|x| ALLCHARS.contains(x))
        {
            Ok(Self(input))
        } else {
            Err(AppError::NNumber(input))
        }
    }
}

// Split this into an enum, Icao, Iata, Other
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Callsign {
    // Could put optional ModelAirline in here?
    Icao((String, String)),
    Iata((String, String)),
    Other(String),
}

impl Callsign {
    pub fn get_suffix(&self) -> Option<String> {
        match self {
            Self::Iata(callsign) | Self::Icao(callsign) => Some(callsign.1.clone()),
            Self::Other(_) => None,
        }
    }
}

impl fmt::Display for Callsign {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Icao(x) | Self::Iata(x) => write!(f, "{}{}", x.0, x.1),
            Self::Other(x) => write!(f, "{x}"),
        }
    }
}

impl Validate for Callsign {
    // Make sure that input is a valid callsign String, validity is [a-z]{4-8}
    // output into Callsign Enum
    fn validate(input: &str) -> Result<Self, AppError> {
        let input = input.to_uppercase();
        if (4..=8).contains(&input.len()) && input.chars().all(|c| valid_char(c, 'z')) {
            let icao = input.split_at(3);
            let iata = input.split_at(2);
            if icao
                .0
                .chars()
                .all(|c: char| c.to_ascii_lowercase().is_ascii_lowercase())
            {
                Ok(Self::Icao((icao.0.to_owned(), icao.1.to_owned())))
            } else if iata.0.chars().all(|c| valid_char(c, 'z')) {
                if let Ok(n_number) = NNumber::validate(&input) {
                    if n_number_to_mode_s(&n_number).is_ok() {
                        return Ok(Self::Other(input));
                    }
                }
                Ok(Self::Iata((iata.0.to_owned(), iata.1.to_owned())))
            } else {
                Ok(Self::Other(input))
            }
        } else {
            Err(AppError::Callsign(input))
        }
    }
}

/// cargo watch -q -c -w src/ -x 'test mod_api_input -- --nocapture'
#[cfg(test)]
#[allow(clippy::pedantic, clippy::unwrap_used)]
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
        let result = Callsign::validate("Aaa1111");
        assert!(result.is_ok());
        let result = result.unwrap();
        match result {
            Callsign::Icao(x) => {
                assert_eq!(x.0, "AAA");
                assert_eq!(x.1, "1111");
            }
            _ => unreachable!(),
        }

        let result = Callsign::validate("Aa1111");
        assert!(result.is_ok());
        let result = result.unwrap();
        match result {
            Callsign::Iata(x) => {
                assert_eq!(x.0, "AA");
                assert_eq!(x.1, "1111");
            }
            _ => unreachable!(),
        }

        let result = Callsign::validate("n1111");
        assert!(result.is_ok());
        let result = result.unwrap();
        match result {
            Callsign::Other(x) => {
                assert_eq!(x, "N1111");
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn mod_api_input_callsign_err() {
        let test = |input: &str| {
            let result = Callsign::validate(input);
            assert!(result.is_err());
            match result.unwrap_err() {
                AppError::Callsign(err) => assert_eq!(err, input.to_uppercase()),
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
            let result = NNumber::validate(input);
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
            let result = NNumber::validate(input);
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
            let result = ModeS::validate(input);
            assert!(result.is_ok());
            assert_eq!(result.unwrap().0, input.to_uppercase());
        };

        test("AaBb12");
        test("C03BF1");
    }

    #[test]
    fn mod_api_input_mode_s_err() {
        let test = |input: &str| {
            let result = ModeS::validate(input);
            assert!(result.is_err());
            match result.unwrap_err() {
                AppError::ModeS(err) => assert_eq!(err, input.to_uppercase()),
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
            let result = Registration::validate(input);
            assert!(result.is_ok());
            assert_eq!(result.unwrap().0, input.to_uppercase());
        };

        test("n1245");
        test("b-818-f");
    }

    #[test]
    fn mod_api_input_registration_err() {
        let test = |input: &str| {
            let result = Registration::validate(input);
            match result.unwrap_err() {
                AppError::Registration(err) => assert_eq!(err, input.to_uppercase()),
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
