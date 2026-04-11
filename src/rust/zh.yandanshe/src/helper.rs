use aidoku::{
	alloc::{String, Vec},
	imports::{
		html::{Document, Html},
		net::{HttpMethod, Request},
	},
	prelude::*,
	AidokuError, Chapter, Result,
};
use aidoku::alloc::string::ToString;
use aes::Aes256;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use cbc::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use serde::Deserialize;

pub const BASE_URL: &str = "https://yandanshe.net";
pub const UA: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Mobile/15E148 Safari/604.1";

const AES_KEY: &[u8; 32] = b"tH1rU6qZ4vU1sK7pN1wO7mX4bY6dQ9gX";

type Aes256CbcDec = cbc::Decryptor<Aes256>;

// ---------- API ----------

#[derive(Deserialize)]
#[serde(untagged)]
enum InfoField {
	Items(Vec<BookItem>),
	#[allow(dead_code)]
	Message(String),
}

#[derive(Deserialize)]
struct RawApiResponse {
	#[serde(default)]
	status: i32,
	info: InfoField,
}

#[derive(Deserialize)]
pub struct BookItem {
	pub id: String,
	pub title: String,
	pub cover_pic: String,
}

#[derive(Deserialize)]
struct ChapterResponse {
	info: String,
	status: i32,
}

fn ajax_post<T: serde::de::DeserializeOwned>(url: &str, body: &str) -> Result<T> {
	Request::new(url, HttpMethod::Post)?
		.header("User-Agent", UA)
		.header("Content-Type", "application/x-www-form-urlencoded")
		.header("X-Requested-With", "XMLHttpRequest")
		.body(body.as_bytes())
		.json_owned()
}

pub fn search_manga(body: &str) -> Result<Vec<BookItem>> {
	let resp: RawApiResponse =
		ajax_post(&format!("{}/index.php?m=&c=mh&a=load_searchpage", BASE_URL), body)?;
	match resp.info {
		InfoField::Items(items) if resp.status == 1 => Ok(items),
		_ => Ok(Vec::new()),
	}
}

pub fn get_html(url: &str) -> Result<Document> {
	Ok(Request::get(url)?.header("User-Agent", UA).html()?)
}

// ---------- Chapters ----------

pub fn get_all_chapters(book_id: &str) -> Result<Vec<Chapter>> {
	let url = format!("{}/index.php?m=&c=book&a=getjino", BASE_URL);
	let prefix = format!("/home/book/inforedit/{}/", book_id);
	let mut chapters: Vec<Chapter> = Vec::new();
	let mut page = 1;

	loop {
		let body = format!("type=mh&id={}&p={}&sort=0", book_id, page);
		let resp: ChapterResponse = ajax_post(&url, &body)?;
		if resp.status == 0 {
			break;
		}
		let doc = Html::parse(resp.info.as_bytes())?;
		let mut found = false;
		if let Some(items) = doc.select("a") {
			for item in items {
				let href = item.attr("href").unwrap_or_default();
				if let Some(rest) = href.strip_prefix(&prefix) {
					let chapter_id = rest.trim_end_matches('/');
					if chapter_id.is_empty() {
						continue;
					}
					found = true;
					let title = item.text().unwrap_or_default().trim().to_string();
					let num = title
						.chars()
						.filter(|c| c.is_ascii_digit())
						.collect::<String>()
						.parse::<f32>()
						.ok();
					chapters.push(Chapter {
						key: format!("{}/{}", book_id, chapter_id),
						title: Some(title),
						chapter_number: num,
						url: Some(format!("{}{}", BASE_URL, href)),
						..Default::default()
					});
				}
			}
		}
		if !found {
			break;
		}
		page += 1;
	}

	chapters.reverse();
	Ok(chapters)
}

// ---------- AES decrypt ----------

pub fn decrypt_images(html: &str) -> Result<Vec<String>> {
	let encrypted = html
		.find("var encryptedData = \"")
		.and_then(|start| {
			let s = &html[start + 21..];
			s.find('"').map(|end| &s[..end])
		})
		.ok_or_else(|| AidokuError::message("encrypted data not found"))?;

	let data = STANDARD
		.decode(encrypted)
		.map_err(|_| AidokuError::message("base64 decode failed"))?;
	if data.len() < 16 {
		return Err(AidokuError::message("data too short"));
	}

	let mut buf = data[16..].to_vec();
	let pt = Aes256CbcDec::new_from_slices(AES_KEY, &data[..16])
		.map_err(|_| AidokuError::message("invalid key/iv"))?
		.decrypt_padded_mut::<Pkcs7>(&mut buf)
		.map_err(|_| AidokuError::message("decrypt failed"))?;

	let json_str =
		core::str::from_utf8(pt).map_err(|_| AidokuError::message("invalid utf8"))?;
	Ok(serde_json::from_str(json_str)?)
}
