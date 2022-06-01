#![allow(unused)]
use lazy_static::lazy_static;

// size of an icao address
const ICAO_SIZE: u8 = 6;
// max size of a N-Number
const NNUMBER_MAX_SIZE: u8 = 6;

//  # alphabet without I and O
const CHARSET: &str = "ABCDEFGHJKLMNPQRSTUVWXYZ";
const DIGITSET: &str = "0123456789";
const HEXSET: &str = "0123456789ABCDEF";

lazy_static! {
    static ref ALLCHARS: String = format!("{}{}", CHARSET, DIGITSET);
    static ref CHARSET_LEN: usize = CHARSET.chars().count();
    // # 601
    static ref SUFFIX_SIZE: usize = 1 + CHARSET.chars().count() * CHARSET.chars().count() + CHARSET.chars().count();
    //  + int(pow(len(charset),2));
    // let bucket4_size = 1 + len(charset) + len(digitset)             # 35
    static ref BUCKET4_SIZE: usize = 1 + CHARSET.chars().count() + DIGITSET.chars().count();
    // let bucket3_size = len(digitset)*bucket4_size + suffix_size     # 951
    static ref BUCKET3_SIZE: usize = DIGITSET.chars().count() * *BUCKET4_SIZE + *SUFFIX_SIZE;
    // let bucket2_size = len(digitset)*(bucket3_size) + suffix_size   # 10111
    static ref BUCKET2_SIZE: usize = DIGITSET.chars().count() * *BUCKET3_SIZE + *SUFFIX_SIZE;
    // let bucket1_size = len(digitset)*(bucket2_size) + suffix_size   # 101711
    static ref BUCKET1_SIZE: usize = DIGITSET.chars().count() * *BUCKET2_SIZE + *SUFFIX_SIZE;
}

fn get_suffix(offset: usize) -> String {
    if offset == 0 {
        return String::new();
    }
    let index = (offset - 1) / (*CHARSET_LEN + 1);
    let char0 = CHARSET.chars().nth(index).unwrap();
    let rem = (offset - 1) % (*CHARSET_LEN + 1);
    if rem == 0 {
        return String::from(char0);
    }
    format!(
        "{}{}",
        char0,
        CHARSET.chars().nth(rem - 1).unwrap()
    )
}

pub fn is_charset(input: &str) -> bool {
    input.chars().all(|c| ALLCHARS.contains(c))
}

fn suffix_offset(offset: &str) -> Option<usize> {
    let offset_len = offset.chars().count();
    if offset_len == 0 {
        return Some(0);
    }

    if offset_len > 2 || !is_charset(offset) {
        return None;
    }

    let first_char = offset.chars().next().unwrap();
    let index = CHARSET.chars().position(|c| c == first_char).unwrap();

    let mut count = (*CHARSET_LEN + 1) as usize * index + 1;

    if offset_len == 2 {
        let second_char = offset.chars().nth(1).unwrap();
        let second_index = CHARSET.chars().position(|c| c == second_char).unwrap();
        count += second_index + 1;
    }

    Some(count)
}


fn create_icao(prefix: &str, count: usize) -> Option<String> {

	// let as_hex = format!("{:x}", count).chars(). [2..].to_owned();
	// let as_hex = format!("{:X}", count).chars().skip(2).collect::<String>();
	let as_hex = format!("{:X}", count);
	if prefix.len() + as_hex.chars().count() > ICAO_SIZE as usize {
		None
	} else {
		Some(format!("{prefix}0{}", as_hex).to_uppercase())
	}
}

fn icao_to_n(mode_s: &str) -> Option<String> {
	let mode_s = mode_s.to_uppercase();

	if mode_s.chars().count() != ICAO_SIZE as usize || !mode_s.starts_with('A') || !is_charset(&mode_s) {
		return None
	}
	let mut output = String::from("N");
	let mode_s_int = usize::from_str_radix(&mode_s[1..], 16).unwrap() -1;

	let digit_1 = mode_s_int / *BUCKET1_SIZE +1;
	let mut rem_1 = mode_s_int % *BUCKET1_SIZE;
	output.push_str(&format!("{}", digit_1));
	if rem_1 < *SUFFIX_SIZE {
		let su = get_suffix(rem_1);
		return Some(format!("{}{}", output, su))
	}

	rem_1 -= *SUFFIX_SIZE;
	
	let digit_2 = rem_1 / *BUCKET2_SIZE;
	let mut rem_2 = rem_1 % *BUCKET2_SIZE;
	output.push_str(&format!("{}", digit_2));
	if rem_2 < *SUFFIX_SIZE {
		return Some(format!("{}{}", output, get_suffix(rem_2)));
	}

	let rem_2 = rem_2 - *SUFFIX_SIZE;
	let digit_3 = rem_2 / *BUCKET3_SIZE;
	let mut rem_3 = rem_2 % *BUCKET3_SIZE;
	output.push_str(&format!("{}", digit_3));
	if rem_3 < *SUFFIX_SIZE {
		return Some(format!("{}{}", output, get_suffix(rem_3)));
	}

	rem_3 -= *SUFFIX_SIZE;
	let digit_4 = rem_3 / *BUCKET4_SIZE;
	let mut rem_4 = rem_3 % *BUCKET4_SIZE;
	output.push_str(&format!("{}", digit_4));

	if rem_4 == 0 {
		return Some(output)
	}


	let final_char = ALLCHARS.chars().nth(rem_4-1).unwrap();
	output.push(final_char);
	Some(output)

}

	// "hello_world".to_owned()
// }
/// http tests - ran via actual requests to a (local) server
/// cargo watch -q -c -w src/ -x 'test n_number_mod -- --test-threads=1 --nocapture'
#[cfg(test)]
mod tests {
    use super::*;

	#[test]
    fn n_number_mod_icao_to_n() {
        // // ""
        // assert_eq!(get_suffix(0), "");
        // // A
        // assert_eq!(get_suffix(1), "A");
        // // AA
        // assert_eq!(get_suffix(2), "AA");
        // // AB
        // assert_eq!(get_suffix(3), "AB");
        // // AC
        // assert_eq!(get_suffix(4), "AC");

		// N29999
        let p = icao_to_n("a0070e");
        println!("p: {:?}", p);

        // // AB
        // let p = get_suffix(3);
        // println!("p: {}", p);

        // // AC
        // let p = get_suffix(4);
        // println!("p: {}", p);

        // // AZ
        // // this is wrong!
        // let p = get_suffix(24);
        // println!("p: {}", p);
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

// cosnt allchars = charset+digitset

// let bucket2_size = len(digitset)*(bucket3_size) + suffix_size   # 10111
// let bucket1_size = len(digitset)*(bucket2_size) + suffix_size   # 101711
