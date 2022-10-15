// Based on
// (c) Guillaume Michel
// https://github.com/guillaumemichel/icao-nnumber_converter
// Licensed under Gnu Public License GPLv3.

// Honestly don't truly understand what is happening in most of these functions
// But it seems to work as expected, although probably inefficient
use std::fmt;

use crate::api::{AppError, ModeS, NNumber};
use lazy_static::lazy_static;

const ICAO_SIZE: usize = 6;

// alphabet without I and O
const ICAO_CHARSET: &str = "ABCDEFGHJKLMNPQRSTUVWXYZ";
const DIGITSET: &str = "0123456789";
const CHARSET_LEN: usize = 24;

lazy_static! {
    /// Uppercase icao charset + digits
    pub static ref ALLCHARS: String = format!("{}{}", ICAO_CHARSET, DIGITSET);
}

const SUFFIX_SIZE: usize = 601;

enum NError {
    CharToDigit,
    FormatModeS,
    FinalChar,
    FirstChar,
    GetIndex,
    GetSuffix,
    SuffixOffset,
}
impl NError {
    fn error(&self) -> AppError {
        AppError::Internal(self.to_string())
    }
}

impl fmt::Display for NError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let disp = match self {
            Self::CharToDigit => "char_not_digit",
            Self::FormatModeS => "format_mode_s",
            Self::FinalChar => "final_char",
            Self::FirstChar => "not_a",
            Self::GetIndex => "get_index",
            Self::GetSuffix => "get_suffix",
            Self::SuffixOffset => "suffix_offset",
        };
        write!(f, "N-Number::{}", disp)
    }
}

enum Bucket {
    One,
    Two,
    Three,
    Four,
}

impl Bucket {
    const fn get(&self) -> usize {
        match self {
            Self::One => 101_711,
            Self::Two => 10_111,
            Self::Three => 951,
            Self::Four => 35,
        }
    }
    const fn extra(&self) -> u32 {
        match self {
            Self::One => 1,
            _ => 0,
        }
    }
}

/// Compute the suffix for the tail number given an offset
/// offset < suffix_size
/// An offset of 0 returns in a valid emtpy suffix
/// A non-zero offset return a string containing one or two character from 'charset'
/// Reverse function of suffix_shift()
/// 0 -> ''
/// 1 -> 'A'
fn get_suffix(offset: usize) -> Result<String, AppError> {
    if offset == 0 {
        return Ok(String::new());
    }
    let index = (offset - 1) / (CHARSET_LEN + 1);

    if let Some(first_char) = ICAO_CHARSET.chars().nth(index) {
        let rem = (offset - 1) % (CHARSET_LEN + 1);
        if rem == 0 {
            return Ok(first_char.to_string());
        }
        ICAO_CHARSET
            .chars()
            .nth(rem - 1)
            .map_or(Err(NError::GetSuffix.error()), |c| {
                Ok(format!("{}{}", first_char, c))
            })
    } else {
        Err(NError::GetSuffix.error())
    }
}

fn suffix_index(offset: &str, index: usize) -> Result<usize, AppError> {
    offset
        .chars()
        .nth(index)
        .map_or(Err(NError::GetIndex.error()), |second_char| {
            ICAO_CHARSET
                .chars()
                .position(|c| c == second_char)
                .map_or_else(|| Err(NError::GetIndex.error()), Ok)
        })
}

/// Compute the offset corresponding to the given alphabetical suffix
/// Reverse function of get_suffix()
/// ''   -> 0
/// 'A'  -> 1
fn suffix_offset(offset: &str) -> Result<usize, AppError> {
    let offset_len = offset.chars().count();
    if offset_len == 0 {
        return Ok(0);
    }
    if offset_len > 2 || !offset.chars().all(|x| ALLCHARS.contains(x)) {
        return Err(NError::SuffixOffset.error());
    }
    let mut count = (CHARSET_LEN + 1) * suffix_index(offset, 0)? + 1;
    if offset_len == 2 {
        count += suffix_index(offset, 1)? + 1;
    }
    Ok(count)
}

/// Creates an american mode_s number composed from the prefix ('a' for USA)
/// and from the given number i
/// The output is an hexadecimal of length 6 starting with the suffix
/// Example: format_mode_s('a', 11) -> "a0000b"
fn format_mode_s(prefix: &str, count: usize) -> Result<ModeS, AppError> {
    let as_hex = format!("{:X}", count);
    let l = prefix.chars().count() + as_hex.chars().count();
    if prefix.len() + as_hex.chars().count() > ICAO_SIZE {
        Err(NError::FormatModeS.error())
    } else {
        let to_fill = format!("{:0^width$}", "", width = ICAO_SIZE - l);
        ModeS::try_from(format!("{prefix}{to_fill}{}", as_hex).to_uppercase())
    }
}

// Convert from ModeS to NNumber, maybe always return a string?
pub fn mode_s_to_n_number(mode_s: &ModeS) -> Result<NNumber, AppError> {
    // N-Numbers only apply to America aircraft, and American aircraft ICAO all start with 'A'
    if !mode_s.to_string().starts_with('A') {
        return Err(NError::FirstChar.error());
    }

    // All N-Numbers start with 'N'
    let mut output = String::from("N");
    // remove the 'A' first char, and convert to hex
    let mut rem = usize::from_str_radix(&mode_s.to_string()[1..], 16)? - 1;

    let calc_rem = |output: &mut String, rem: usize, bucket: Bucket| -> usize {
        let digit = rem / bucket.get() + bucket.extra() as usize;
        let rem = rem % bucket.get();
        output.push_str(&digit.to_string());
        rem
    };

    for bucket in [Bucket::One, Bucket::Two, Bucket::Three, Bucket::Four] {
        if let Bucket::Four = bucket {
            rem = calc_rem(&mut output, rem, Bucket::Four);
            if rem == 0 {
                return NNumber::try_from(output);
            }
        } else {
            rem = calc_rem(&mut output, rem, bucket);
            if rem < SUFFIX_SIZE {
                return NNumber::try_from(format!("{}{}", output, get_suffix(rem)?));
            }
            rem -= SUFFIX_SIZE;
        }
    }

    ALLCHARS.chars().nth(rem - 1).map_or_else(
        || Err(NError::FinalChar.error()),
        |final_char| {
            output.push(final_char);
            NNumber::try_from(output)
        },
    )
}

fn n_number_index(n_number: &str, index: usize) -> Result<char, AppError> {
    n_number
        .chars()
        .nth(index)
        .map_or_else(|| Err(NError::CharToDigit.error()), Ok)
}

fn calc_count(n_number: &str, index: usize, bucket: Option<Bucket>) -> Result<usize, AppError> {
    let char = n_number_index(n_number, index)?;
    bucket.map_or_else(
        || {
            ALLCHARS
                .chars()
                .position(|x| x == char)
                .map_or_else(|| Err(NError::GetIndex.error()), |pos| Ok(pos + 1))
        },
        |bucket| {
            char.to_digit(10).map_or_else(
                || Err(NError::CharToDigit.error()),
                |mut value| {
                    value -= bucket.extra() as u32;
                    let output = match bucket {
                        Bucket::One => value as usize * bucket.get(),
                        _ => value as usize * bucket.get() + SUFFIX_SIZE,
                    };
                    Ok(output)
                },
            )
        },
    )
}

/// Convert a Tail Number (N-Number) to the corresponding ICAO address
/// Only works with US registrations (ICAOS starting with 'a' and tail number starting with 'N')
/// Return None for invalid parameter
/// Return the ICAO address associated with the given N-Number in string format on success
pub fn n_number_to_mode_s(n_number: &NNumber) -> Result<ModeS, AppError> {
    let prefix = "a";
    let mut count = 0;

    // this is messy?
    let mut n_number = &n_number.to_string()[0..];

    if n_number.chars().count() > 1 {
        n_number = &n_number[1..];
        count += 1;
        for index in 0..n_number.chars().count() {
            if let Some(char_index) = n_number.chars().nth(index) {
                if index == 4 {
                    count += calc_count(n_number, index, None)?;
                } else if ICAO_CHARSET.contains(char_index) {
                    count += suffix_offset(&n_number[index..])?;
                    break;
                } else if index == 0 {
                    count += calc_count(n_number, index, Some(Bucket::One))?;
                } else if index == 1 {
                    count += calc_count(n_number, index, Some(Bucket::Two))?;
                } else if index == 2 {
                    count += calc_count(n_number, index, Some(Bucket::Three))?;
                } else if index == 3 {
                    count += calc_count(n_number, index, Some(Bucket::Four))?;
                }
            } else {
                return Err(NError::FormatModeS.error());
            }
        }
    }
    format_mode_s(prefix, count)
}

/// cargo watch -q -c -w src/ -x 'test n_number_mod -- --nocapture'
#[cfg(test)]
#[allow(clippy::pedantic, clippy::nursery, clippy::unwrap_used)]
mod tests {
    use super::*;

    // This will create every valid American, as in starts with 'A', mode_s
    fn gen_all_mode_s() -> Vec<ModeS> {
        let mut output = vec![];
        for i in 1..=915_399 {
            let mode_s = format_mode_s("a", i).unwrap();
            output.push(mode_s);
        }
        output
    }

    #[test]
    fn n_number_mod_mode_s_to_n() {
        let test = |mode_s: &str, n_number: &str| {
            let mode_s = ModeS::try_from(mode_s).unwrap();
            let result = mode_s_to_n_number(&mode_s);
            assert_eq!(result.unwrap().to_string(), n_number);
        };

        test("a00001", "N1");
        test("a00724", "N1000Z");
        test("a00725", "N10000");

        test("a00726", "N10001");
        test("a00727", "N10002");
        test("a0072e", "N10009");

        test("a0072f", "N1001");
        test("a00730", "N1001A");
        test("a00731", "N1001B");

        test("a00751", "N10019");
        test("a00752", "N1002");
        test("a00869", "N10099");

        test("a0086a", "N101");
        test("a0086b", "N101A");
        test("a0086c", "N101AA");

        test("a00c20", "N10199");
        test("a00c21", "N102");
        test("a00c22", "N102A");

        test("a029d8", "N10999");
        test("a029d9", "N11");
        test("a029da", "N11A");

        test("a029db", "N11AA");
        test("a05157", "N11999");
        test("a05158", "N12");

        test("a18d4f", "N19999");
        test("a18d50", "N2");
        test("a18d51", "N2A");

        test("a18d52", "N2AA");
        test("A3C9A1", "N343NB");
        test("A403B3", "N358NB");

        test("A61D3E", "N493WN");
        test("A7DE57", "N606JF");
        test("AA0AAB", "N746UW");

        test("AA7548", "N773MJ");
        test("AC6DE9", "N90MC");
        test("adf7c7", "N99999");
    }

    #[test]
    /// Create every possible valid mode_s, and make sure can be converted to N-Number
    fn n_number_mod_every_mode_s_to_n() {
        let test = |mode_s: &ModeS| {
            let result = mode_s_to_n_number(mode_s);
            assert!(result.is_ok());
        };

        let all_possible_mode_s = gen_all_mode_s();
        for mode_s in all_possible_mode_s {
            test(&mode_s);
        }
    }

    #[test]
    /// Only works with American mode_s, which start with 'A'
    fn n_number_mod_mode_s_to_n_err() {
        let test = |mode_s: &str| {
            let mode_s = ModeS::try_from(mode_s).unwrap();
            let result = mode_s_to_n_number(&mode_s);
            assert!(result.is_err());
        };

        test("B00001");
        test("F00724");
        test("C00725");
        test("E00725");
    }

    #[test]
    fn n_number_mod_n_to_mode_s() {
        let test = |n_number: &str, mode_s: &str| {
            let result = n_number_to_mode_s(&NNumber::try_from(n_number).unwrap());
            assert_eq!(result.unwrap().to_string(), mode_s);
        };

        test("N1", "A00001");
        test("N1000Z", "A00724");
        test("N10000", "A00725");

        test("N10001", "A00726");
        test("N10002", "A00727");
        test("N10009", "A0072E");

        test("N1001", "A0072F");
        test("N1001A", "A00730");
        test("N1001B", "A00731");

        test("N10019", "A00751");
        test("N1002", "A00752");
        test("N10099", "A00869");

        test("N101", "A0086A");
        test("N101A", "A0086B");
        test("N101AA", "A0086C");

        test("N10199", "A00C20");
        test("N102", "A00C21");
        test("N102A", "A00C22");

        test("N10999", "A029D8");
        test("N11", "A029D9");
        test("N11A", "A029DA");

        test("N11AA", "A029DB");
        test("N11999", "A05157");
        test("N12", "A05158");

        test("N19999", "A18D4F");
        test("N2", "A18D50");
        test("N2A", "A18D51");

        test("N2AA", "A18D52");
        test("N343NB", "A3C9A1");
        test("N358NB", "A403B3");

        test("N493WN", "A61D3E");
        test("N606JF", "A7DE57");
        test("N746UW", "AA0AAB");

        test("N773MJ", "AA7548");
        test("N90MC", "AC6DE9");
        test("N99999", "ADF7C7");
    }
}
