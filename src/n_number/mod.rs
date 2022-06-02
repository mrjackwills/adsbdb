// Based on
// (c) Guillaume Michel
// https://github.com/guillaumemichel/icao-nnumber_converter
// Licensed under Gnu Public License GPLv3.

#![allow(unused)]

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
        Err(AppError::Internal(String::from("icao_to_n::get_suffix")))
    }
}

fn get_index(offset: &str, index: usize) -> Result<usize, AppError> {
    let second_char = offset.chars().nth(index).unwrap();
    if let Some(index) = ICAO_CHARSET.chars().position(|c| c == second_char) {
        Ok(index)
    } else {
        Err(AppError::Internal(String::from("icao_to_n::suffix_offset")))
    }
}

fn suffix_offset(offset: &str) -> Result<usize, AppError> {
    let offset_len = offset.chars().count();
    if offset_len == 0 {
        return Ok(0);
    }

    if offset_len > 2 || !is_hex(offset) {
        return Err(AppError::Internal(String::from("icao_to_n::suffix_offset")));
    }

    let index = get_index(offset, 0)?;
    let mut count = (CHARSET_LEN + 1) * index + 1;

    if offset_len == 2 {
        count += get_index(offset, 1)? + 1;
    }

    Ok(count)
}

fn create_icao(prefix: &str, count: usize) -> Result<String, AppError> {
    let as_hex = format!("{:X}", count);
    if prefix.len() + as_hex.chars().count() > ICAO_SIZE {
		Err(AppError::Internal(String::from("icao_to_n::create_icao")))
    } else {
        Ok(format!("{prefix}0{}", as_hex).to_uppercase())
    }
}

// Get the N-Number from a given ICOA string
fn icao_to_n(mode_s: &ModeS) -> Result<String, AppError> {

	// N-Numbers only apply to America aircraft, and American aircraft ICAO all start with 'A'
    if !mode_s.mode_s.starts_with('A') {
        return Err(AppError::Internal(String::from("icao_to_n::A")))
    }

	// All N-Numbers start with 'N'
    let mut output = String::from("N");
	// remove the 'A' first char, and convert to hex
	let mode_s_int = usize::from_str_radix(&mode_s.mode_s[1..], 16)? -1;


	let calc_rem = |output: &mut String, rem: usize, bucket: Bucket| -> usize {
		let digit = rem / bucket.get() + bucket.extra();
		let mut rem = rem % bucket.get();
		output.push_str(&format!("{}", digit));
		rem
	};

	let mut rem = calc_rem(&mut output, mode_s_int, Bucket::One);

    if rem < SUFFIX_SIZE {
        return Ok(format!("{}{}", output, get_suffix(rem)?));
    }
    rem -= SUFFIX_SIZE;

	let mut rem = calc_rem(&mut output, rem, Bucket::Two);
    if rem < SUFFIX_SIZE {
        return Ok(format!("{}{}", output, get_suffix(rem)?));
    }
    rem -= SUFFIX_SIZE;

	let mut rem = calc_rem(&mut output, rem, Bucket::Three);
    if rem < SUFFIX_SIZE {
        return Ok(format!("{}{}", output, get_suffix(rem)?));
    }
    rem -= SUFFIX_SIZE;
	
	let mut rem = calc_rem(&mut output, rem, Bucket::Four);
    if rem == 0 {
        return Ok(output);
    }

    let final_char = ALLCHARS.chars().nth(rem - 1).unwrap();
    output.push(final_char);
    Ok(output)
}

/// cargo watch -q -c -w src/ -x 'test n_number_mod -- --test-threads=1 --nocapture'
#[cfg(test)]
mod tests {
    use super::*;

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
