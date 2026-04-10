use aidoku::{
	alloc::String,
	helpers::uri::encode_uri,
	imports::{html::Document, net::Request},
	prelude::*,
	Result,
};

use crate::crypto;

pub const WWW_URL: &str = "https://www.mangacopy.com";
pub const API_URL: &str = "https://api.mangacopy.com/api/v3";
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36";

pub fn decrypt(text: String, key: String) -> String {
	let text = text.as_bytes();
	let key = key.as_bytes();
	let key: &[u8; 16] = key.try_into().unwrap();
	let iv = &text[..16];
	let cipher = &text[16..];
	let cipher = hex::decode(cipher).unwrap();
	let pt = crypto::decrypt(&cipher, key, iv).unwrap();
	String::from_utf8_lossy(&pt).replace('\u{0}', "")
}

pub fn get_text(url: &str) -> Result<String> {
	Ok(Request::get(url)?.header("User-Agent", UA).string()?)
}

pub fn get_html(url: &str) -> Result<Document> {
	Ok(Request::get(url)?.header("User-Agent", UA).html()?)
}

pub fn get_json<T: serde::de::DeserializeOwned>(url: &str) -> Result<T> {
	let request = Request::get(url)?;
	let request = if url.starts_with(WWW_URL) {
		request.header("User-Agent", UA)
	} else {
		request
			.header("User-Agent", "COPY/2.3.1")
			.header("version", "2.3.1")
			.header("platform", "3")
			.header("region", "1")
			.header("webp", "1")
	};
	Ok(request.json_owned()?)
}

pub fn gen_explore_url(theme: &str, top: &str, ordering: &str, page: i32) -> String {
	format!(
		"{}/comics?theme={}&top={}&ordering={}&limit=50&offset={}",
		API_URL,
		theme,
		top,
		ordering,
		(page - 1) * 50,
	)
}

pub fn gen_search_url(query: &str, page: i32) -> String {
	format!(
		"{}/search/comic?q={}&q_type=&limit=20&offset={}",
		API_URL,
		encode_uri(query),
		(page - 1) * 20
	)
}

pub fn gen_rank_url(date_type: &str, page: i32) -> String {
	format!(
		"{}/ranks?date_type={}&limit=30&offset={}",
		API_URL,
		date_type,
		(page - 1) * 30,
	)
}

pub fn gen_recs_url(page: i32) -> String {
	format!(
		"{}/recs?pos=3200102&limit=30&offset={}",
		API_URL,
		(page - 1) * 30,
	)
}

pub fn gen_newest_url(page: i32) -> String {
	format!(
		"{}/update/newest?limit=30&offset={}",
		API_URL,
		(page - 1) * 30,
	)
}

pub fn gen_manga_url(id: &str) -> String {
	format!("{}/comic/{}", WWW_URL, id)
}

pub fn gen_chapter_list_url(id: &str) -> String {
	format!("{}/comicdetail/{}/chapters", WWW_URL, id)
}

pub fn gen_chapter_url(manga_id: &str, chapter_id: &str) -> String {
	format!("{}/comic/{}/chapter/{}", WWW_URL, manga_id, chapter_id)
}

pub fn gen_page_list_url(manga_id: &str, chapter_id: &str) -> String {
	format!("{}/comic/{}/chapter/{}", WWW_URL, manga_id, chapter_id)
}
