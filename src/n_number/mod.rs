// Based on
// (c) Guillaume Michel
// https://github.com/guillaumemichel/icao-nnumber_converter
// Licensed under Gnu Public License GPLv3.

#![allow(unused)]

use std::fmt;

use crate::api::{is_hex, AppError, ModeS};
use anyhow::Result;
use lazy_static::lazy_static;

// size of an icao address
const ICAO_SIZE: usize = 6;
// max size of a N-Number
const NNUMBER_MAX_SIZE: usize = 6;

// alphabet without I and O
const ICAO_CHARSET: &str = "ABCDEFGHJKLMNPQRSTUVWXYZ";
const DIGITSET: &str = "0123456789";
const CHARSET_LEN: usize = 24;

lazy_static! {
    // repalce with is ascii digits!
    static ref ALLCHARS: String = format!("{}{}", ICAO_CHARSET, DIGITSET);
}

// suffix_size = 1 + len(charset) + int(pow(len(charset),2))
// static ref SUFFIX_SIZE: usize = 1 + CHARSET.chars().count() * CHARSET.chars().count() + CHARSET.chars().count();
const SUFFIX_SIZE: usize = 601;

// let bucket4_size = 1 + len(charset) + len(digitset)             # 35
// static ref BUCKET_4: usize = 1 + CHARSET.chars().count() + DIGITSET.chars().count();
// const BUCKET_4: usize = 35;

// let bucket3_size = len(digitset)*bucket4_size + suffix_size     # 951
// static ref BUCKET_3: usize = DIGITSET.chars().count() * BUCKET_4 + SUFFIX_SIZE;
// const BUCKET_3: usize = 951;

// let bucket2_size = len(digitset)*(bucket3_size) + suffix_size   # 10111
// static ref BUCKET_2 ref BUCKET2_SIZE: usize = DIGITSET.chars().count() * BUCKET_3 + SUFFIX_SIZE;
// const BUCKET_2: usize = 10111;

// let bucket1_size = len(digitset)*(bucket2_size) + suffix_size   # 101711
// static ref BUCKET1_SIZE: usize = DIGITSET.chars().count() * BUCKET_2 + SUFFIX_SIZE;
// const BUCKET_1: usize = 101711;

enum NNumberError {
    GetSuffix,
    GetIndex,
    SuffixOffset,
    CreateICAO,
    FirstChar,
    FinalChar,
}
impl NNumberError {
    fn error(&self) -> AppError {
        AppError::Internal(self.to_string())
    }
}

impl fmt::Display for NNumberError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let disp = match self {
            Self::GetSuffix => "get_suffix",
            Self::GetIndex => "get_index",
            Self::SuffixOffset => "suffix_offset",
            Self::CreateICAO => "create_icao",
            Self::FirstChar => "not_a",
            Self::FinalChar => "final_char",
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
    fn get(&self) -> usize {
        match self {
            Self::One => 101711,
            Self::Two => 10111,
            Self::Three => 951,
            Self::Four => 35,
        }
    }
    fn extra(&self) -> usize {
        match self {
            Self::One => 1,
            _ => 0,
        }
    }
}

fn get_index(offset: &str, index: usize) -> Result<usize, AppError> {
    let second_char = offset.chars().nth(index).unwrap();
    if let Some(index) = ICAO_CHARSET.chars().position(|c| c == second_char) {
        Ok(index)
    } else {
        Err(NNumberError::GetIndex.error())
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
            return Ok(String::from(first_char));
        }
        Ok(format!(
            "{}{}",
            first_char,
            ICAO_CHARSET.chars().nth(rem - 1).unwrap()
        ))
    } else {
        Err(NNumberError::GetSuffix.error())
    }
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
        return Err(NNumberError::SuffixOffset.error());
    }
    let index = get_index(offset, 0)?;
    let mut count = (CHARSET_LEN + 1) * index + 1;
    if offset_len == 2 {
        count += get_index(offset, 1)? + 1;
    }
    Ok(count)
}


/// Creates an american icao number composed from the prefix ('a' for USA)
/// and from the given number i
/// The output is an hexadecimal of length 6 starting with the suffix
/// Example: create_icao('a', 11) -> "a0000b"
fn create_icao(prefix: &str, count: usize) -> Result<String, AppError> {
    let as_hex = format!("{:X}", count);
	let l = prefix.chars().count() + as_hex.chars().count();
    if prefix.len() + as_hex.chars().count() > ICAO_SIZE {
        Err(NNumberError::CreateICAO.error())
    } else {
		let to_fill = format!("{:0^width$}", "", width=ICAO_SIZE-l);
        Ok(format!("{prefix}{to_fill}{}", as_hex).to_uppercase())
    }
}

// Maybe just string Option<String>?
// Get the N-Number from a given ICAO string
fn icao_to_n(mode_s: &ModeS) -> Result<String, AppError> {
    // N-Numbers only apply to America aircraft, and American aircraft ICAO all start with 'A'
    if !mode_s.mode_s.starts_with('A') {
        return Err(NNumberError::FirstChar.error());
    }

    // All N-Numbers start with 'N'
    let mut output = String::from("N");
    // remove the 'A' first char, and convert to hex
    let mut rem = usize::from_str_radix(&mode_s.mode_s[1..], 16)? - 1;

    let calc_rem = |output: &mut String, rem: usize, bucket: Bucket| -> usize {
        let digit = rem / bucket.get() + bucket.extra();
        let mut rem = rem % bucket.get();
        output.push_str(&format!("{}", digit));
        rem
    };

    for bucket in [Bucket::One, Bucket::Two, Bucket::Three, Bucket::Four] {
        match bucket {
            Bucket::Four => {
                rem = calc_rem(&mut output, rem, Bucket::Four);
                if rem == 0 {
                    return Ok(output);
                }
            }
            _ => {
                rem = calc_rem(&mut output, rem, bucket);
                if rem < SUFFIX_SIZE {
                    return Ok(format!("{}{}", output, get_suffix(rem)?));
                }
                rem -= SUFFIX_SIZE;
            }
        }
    }

    if let Some(final_char) = ALLCHARS.chars().nth(rem - 1) {
        output.push(final_char);
        Ok(output)
    } else {
        Err(NNumberError::FinalChar.error())
    }
}

/// Convert a Tail Number (N-Number) to the corresponding ICAO address
/// Only works with US registrations (ICAOS starting with 'a' and tail number starting with 'N')
/// Return None for invalid parameter
/// Return the ICAO address associated with the given N-Number in string format on success
fn n_to_icao(mut n_number: &str) -> Option<String> {
    let len_n_number = n_number.chars().count();

    // check n number starts with N, and is len 2-6, and only has valid chars from ALLCHARS
    if !n_number.to_uppercase().starts_with('N')
        || !(2..=NNUMBER_MAX_SIZE).contains(&len_n_number)
        || !n_number.chars().all(|x| ALLCHARS.contains(x))
    {
        return None;
    }

	// println!("here");

    let prefix = "a";
    let mut count = 0;

	
	if n_number.chars().count() > 1 {
		n_number = &n_number[1..];
		count +=1;
		for index in (0..n_number.chars().count()) {
			if index == NNUMBER_MAX_SIZE - 2 {
				let abc = n_number.chars().nth(index).unwrap();
				let to_add = ALLCHARS.chars().position(|x| x == abc).unwrap() +1;
				count += to_add;
			} else if ICAO_CHARSET.contains(n_number.chars().nth(index).unwrap()) {
				let a = suffix_offset(&n_number[index..]).unwrap();
				count += a;
				break;
			} else if index == 0 {
				let y = n_number.chars().nth(index).unwrap().to_digit(10).unwrap() as usize - 1;
				count += y * Bucket::One.get();
			} else if index == 1 {
				let y = n_number.chars().nth(index).unwrap().to_digit(10).unwrap() as usize;
				count += y * Bucket::Two.get() + SUFFIX_SIZE;
			} else if index == 2 {
				let y = n_number.chars().nth(index).unwrap().to_digit(10).unwrap() as usize;
				count += y * Bucket::Three.get() + SUFFIX_SIZE;
			} else if index == 3 {
				let y = n_number.chars().nth(index).unwrap().to_digit(10).unwrap() as usize;
				count += y * Bucket::Four.get() + SUFFIX_SIZE;
			}
		}
	} else{
		count = 1;
	}
	let pp = create_icao(prefix, count).unwrap();

    Some(pp)
}


// 1 max 915399






/// cargo watch -q -c -w src/ -x 'test n_number_mod -- --nocapture'
#[cfg(test)]
mod tests {
    use super::*;

	// #[test]
	// fn n_number_mod() {
	// 	tester()
	// }

	// This will create every known icao to number
fn gen_all_icao() -> Vec<(String, String)>{

	let mut output = vec![];
	for i in (1..=915399){
		let icao = create_icao("a", i).unwrap();
		// println!("{}", a)

		let mode_s = ModeS::new(icao.to_owned()).unwrap();
		let n_number = icao_to_n(&mode_s).unwrap();

		output.push((mode_s.mode_s, n_number));
	}
	output

}

    #[test]
    fn n_number_mod_icao_to_n() {
        let test = |icao: &str, n_number: &str| {
            let mode_s = ModeS::new(icao.to_owned()).unwrap();
            let result = icao_to_n(&mode_s);
            assert_eq!(result.unwrap(), n_number);
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
    fn n_number_mod_n_to_icao() {
        let test = |n_number: &str, icao: &str| {
            // let mode_s = ModeS::new(icao.to_owned()).unwrap();
            let result = n_to_icao(n_number);
            assert_eq!(result.unwrap(), icao);
        };

        test("N1", "A00001");
        test( "N1000Z", "A00724");
        test( "N10000", "A00725");
        test( "N10001", "A00726");
        test( "N10002", "A00727");
        test( "N10009", "A0072E");
        test( "N1001", "A0072F");
        test( "N1001A", "A00730");
        test( "N1001B", "A00731");
        test( "N10019", "A00751");
        test( "N1002", "A00752");
        test( "N10099", "A00869");
        test( "N101", "A0086A");
        test( "N101A", "A0086B");

        test( "N101AA", "A0086C");
        test( "N10199", "A00C20");
        test( "N102", "A00C21");

        test( "N102A", "A00C22");
        test( "N10999", "A029D8");
        test( "N11", "A029D9");

        test( "N11A", "A029DA");
        test( "N11AA", "A029DB");
        test( "N11999", "A05157");

        test( "N12", "A05158");
        test( "N19999", "A18D4F");
        test( "N2", "A18D50");

        test( "N2A", "A18D51");
        test( "N2AA", "A18D52");
        test( "N343NB", "A3C9A1");

        test( "N358NB", "A403B3");
        test( "N493WN", "A61D3E");
        test( "N606JF", "A7DE57");

        test( "N746UW", "AA0AAB");
        test( "N773MJ", "AA7548");
        test( "N90MC", "AC6DE9");

        test( "N99999", "ADF7C7");
    }

    // #[test]
    // fn n_number_mod_suffix_offset() {
    //     // // ""
    //     // assert_eq!(get_suffix(0), "");
    //     // // A
    //     // assert_eq!(get_suffix(1), "A");
    //     // // AA
    //     // assert_eq!(get_suffix(2), "AA");
    //     // // AB
    //     // assert_eq!(get_suffix(3), "AB");
    //     // // AC
    //     // assert_eq!(get_suffix(4), "AC");

    //     let p = create_icao("a", 11200);
    //     println!("p: {:?}", p);

    //     // // AB
    //     // let p = get_suffix(3);
    //     // println!("p: {}", p);

    //     // // AC
    //     // let p = get_suffix(4);
    //     // println!("p: {}", p);

    //     // // AZ
    //     // // this is wrong!
    //     // let p = get_suffix(24);
    //     // println!("p: {}", p);
    // }

    // #[test]
    // fn n_number_mod_lazy_static() {
    // 	assert_eq!(*CHARSET_LEN, 24);
    // 	assert_eq!(*SUFFIX_SIZE, 601);
    // 	assert_eq!(*BUCKET4_SIZE, 35);
    // 	assert_eq!(*BUCKET3_SIZE, 951);
    // 	assert_eq!(*BUCKET2_SIZE, 10111);
    // 	assert_eq!(*BUCKET1_SIZE, 101711);
    // }

    // #[test]
    // fn n_number_mod_get_suffix() {
    //     // ""
    //     assert_eq!(get_suffix(0), "");
    //     // A
    //     assert_eq!(get_suffix(1), "A");
    //     // AA
    //     assert_eq!(get_suffix(2), "AA");
    //     // AB
    //     assert_eq!(get_suffix(3), "AB");
    //     // AC
    //     assert_eq!(get_suffix(4), "AC");

    //     let p = get_suffix(600);
    //     println!("p: {}", p);

    //     // AB
    //     let p = get_suffix(3);
    //     println!("p: {}", p);

    //     // AC
    //     let p = get_suffix(4);
    //     println!("p: {}", p);

    //     // AZ
    //     // this is wrong!
    //     let p = get_suffix(24);
    //     println!("p: {}", p);
    // }
    // #[test]
    // fn n_number_mod_suffix_offset() {
    //     // // ""
    //     // assert_eq!(get_suffix(0), "");
    //     // // A
    //     // assert_eq!(get_suffix(1), "A");
    //     // // AA
    //     // assert_eq!(get_suffix(2), "AA");
    //     // // AB
    //     // assert_eq!(get_suffix(3), "AB");
    //     // // AC
    //     // assert_eq!(get_suffix(4), "AC");

    //     let p = suffix_offset("ZZ");
    //     println!("p: {:?}", p);

    //     // // AB
    //     // let p = get_suffix(3);
    //     // println!("p: {}", p);

    //     // // AC
    //     // let p = get_suffix(4);
    //     // println!("p: {}", p);

    //     // // AZ
    //     // // this is wrong!
    //     // let p = get_suffix(24);
    //     // println!("p: {}", p);
    // }
}
