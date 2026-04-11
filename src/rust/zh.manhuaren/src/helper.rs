use aidoku::{
	alloc::{String, Vec},
	helpers::uri::encode_uri,
	imports::{
		html::Document,
		net::{HttpMethod, Request},
	},
	prelude::*,
	Result,
};
use aidoku::alloc::string::ToString;

const BASE_URL: &str = "https://www.manhuaren.com";
pub const UA: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Mobile/15E148 Safari/604.1";

pub fn post_json<T: serde::de::DeserializeOwned>(body: &str) -> Result<T> {
	let url = format!("{}/dm5.ashx?d=1", BASE_URL);
	Request::new(&url, HttpMethod::Post)?
		.header("User-Agent", UA)
		.header("Content-Type", "application/x-www-form-urlencoded")
		.body(body.as_bytes())
		.json_owned()
}

pub fn search_html(query: &str) -> Result<Document> {
	let url = format!(
		"{}/search?title={}&language=1",
		BASE_URL,
		encode_uri(query.to_string())
	);
	get_html(&url)
}

pub fn get_html(url: &str) -> Result<Document> {
	Ok(Request::get(url)?.header("User-Agent", UA).html()?)
}

pub fn get_text(url: &str) -> Result<String> {
	Ok(Request::get(url)?.header("User-Agent", UA).string()?)
}

pub fn manga_url(slug: &str) -> String {
	format!("{}/{}/", BASE_URL, slug)
}

pub fn chapter_url(key: &str) -> String {
	format!("{}/{}/", BASE_URL, key)
}

// Dean Edwards JavaScript unpacker — extract image URLs from packed eval script
pub fn unpack_images(html: &str) -> Vec<String> {
	let start = match html.find("}('") {
		Some(pos) => pos + 3,
		None => return Vec::new(),
	};
	let rest = &html[start..];

	// Extract packed string, handling \' escapes
	let mut packed = String::new();
	let mut pos = 0;
	let bytes = rest.as_bytes();
	while pos < bytes.len() {
		if bytes[pos] == b'\\' && pos + 1 < bytes.len() {
			packed.push(bytes[pos + 1] as char);
			pos += 2;
		} else if bytes[pos] == b'\'' {
			pos += 1;
			break;
		} else {
			packed.push(bytes[pos] as char);
			pos += 1;
		}
	}

	let after = &rest[pos..];

	let base: usize = after
		.split(',')
		.nth(1)
		.and_then(|s| s.trim().parse().ok())
		.unwrap_or(36);

	let split_marker = "'.split('|')";
	let kw_end = match after.find(split_marker) {
		Some(p) => p,
		None => return Vec::new(),
	};
	let kw_start = match after[..kw_end].rfind('\'') {
		Some(p) => p + 1,
		None => return Vec::new(),
	};
	let keywords: Vec<&str> = after[kw_start..kw_end].split('|').collect();

	// Replace word tokens with keywords
	let mut result = String::new();
	let mut word = String::new();
	for ch in packed.chars() {
		if ch.is_alphanumeric() || ch == '_' {
			word.push(ch);
		} else {
			if !word.is_empty() {
				push_word(&mut result, &word, base, &keywords);
				word.clear();
			}
			result.push(ch);
		}
	}
	if !word.is_empty() {
		push_word(&mut result, &word, base, &keywords);
	}

	// Extract URLs from newImgs=[...]
	let imgs = match result.find("newImgs") {
		Some(p) => p,
		None => return Vec::new(),
	};
	let bracket_start = match result[imgs..].find('[') {
		Some(p) => imgs + p + 1,
		None => return Vec::new(),
	};
	let bracket_end = match result[bracket_start..].find(']') {
		Some(p) => bracket_start + p,
		None => return Vec::new(),
	};

	result[bracket_start..bracket_end]
		.split('\'')
		.enumerate()
		.filter_map(|(i, s)| {
			if i % 2 == 1 && !s.is_empty() {
				Some(String::from(s))
			} else {
				None
			}
		})
		.collect()
}

fn push_word(result: &mut String, word: &str, base: usize, keywords: &[&str]) {
	if let Some(idx) = decode_base(word, base) {
		if idx < keywords.len() && !keywords[idx].is_empty() {
			result.push_str(keywords[idx]);
			return;
		}
	}
	result.push_str(word);
}

fn decode_base(s: &str, base: usize) -> Option<usize> {
	let mut n = 0usize;
	for ch in s.chars() {
		let digit = match ch {
			'0'..='9' => ch as usize - '0' as usize,
			'a'..='z' => ch as usize - 'a' as usize + 10,
			'A'..='Z' => ch as usize - 'A' as usize + 36,
			_ => return None,
		};
		if digit >= base {
			return None;
		}
		n = n * base + digit;
	}
	Some(n)
}
