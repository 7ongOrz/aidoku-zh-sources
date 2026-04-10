use aidoku::{
	alloc::{String, Vec},
	helpers::uri::{encode_uri, QueryParameters},
	imports::{
		defaults::defaults_get,
		net::Request,
	},
	prelude::*,
	Result,
};
use aidoku::alloc::string::ToString;
use md5::compute;

const WWW_URL: &str = "https://www.manhuaren.com";
const API_URL: &str = "https://mangaapi.manhuaren.com";

const GSN_KEY: &str = "4e0a48e1c0b54041bce9c8f0e036124d";

pub fn md5_hash(text: &str) -> String {
	format!("{:x}", compute(text))
}

pub fn get_json<T: serde::de::DeserializeOwned>(url: &str) -> Result<T> {
	let token = defaults_get::<String>("token").unwrap_or_default();
	Request::get(url)?
		.header("Authorization", &format!("YINGQISTS2 {}", token))
		.json_owned()
}

pub fn gen_gsn_hash(mut params: Vec<(String, String)>) -> String {
	let mut hash = String::new();

	params.sort_by(|a, b| a.0.cmp(&b.0));
	hash.push_str(GSN_KEY);
	hash.push_str("GET");

	for param in params {
		hash.push_str(&param.0);
		hash.push_str(&encode_uri(param.1));
	}

	hash.push_str(GSN_KEY);

	md5_hash(&hash)
}

pub fn gen_query_string(mut params: Vec<(String, String)>) -> String {
	let uid = defaults_get::<String>("uid").unwrap_or_default();

	params.push((String::from("gak"), String::from("ios_manhuaren2")));
	params.push((String::from("gft"), String::from("json")));
	params.push((String::from("gui"), uid));
	params.push((String::from("gsn"), gen_gsn_hash(params.clone())));

	let mut query_params = QueryParameters::new();

	for param in params {
		query_params.set(&param.0, Some(&param.1));
	}

	query_params.to_string()
}

pub fn gen_explore_url(category: &str, status: &str, sort: &str, page: i32) -> String {
	let mut params: Vec<(String, String)> = Vec::new();

	params.push((String::from("subCategoryType"), String::from("0")));
	params.push((String::from("subCategoryId"), category.to_string()));
	params.push((String::from("status"), status.to_string()));
	params.push((String::from("sort"), sort.to_string()));
	params.push((String::from("start"), ((page - 1) * 20).to_string()));
	params.push((String::from("limit"), String::from("20")));

	format!(
		"{}/v2/manga/getCategoryMangas?{}",
		API_URL,
		gen_query_string(params)
	)
}

pub fn gen_search_url(query: &str, page: i32) -> String {
	let mut params: Vec<(String, String)> = Vec::new();

	params.push((String::from("keywords"), query.to_string()));
	params.push((String::from("start"), ((page - 1) * 20).to_string()));
	params.push((String::from("limit"), String::from("20")));

	format!(
		"{}/v1/search/getSearchManga?{}",
		API_URL,
		gen_query_string(params)
	)
}

pub fn gen_manga_details_url(id: &str) -> String {
	let mut params: Vec<(String, String)> = Vec::new();

	params.push((String::from("mangaId"), id.to_string()));

	format!(
		"{}/v1/manga/getDetail?{}",
		API_URL,
		gen_query_string(params)
	)
}

pub fn gen_chapter_url(chapter_id: &str) -> String {
	format!("{}/m{}/", WWW_URL, chapter_id)
}

pub fn gen_page_list_url(manga_id: &str, chapter_id: &str) -> String {
	let mut params: Vec<(String, String)> = Vec::new();

	params.push((String::from("mangaId"), manga_id.to_string()));
	params.push((String::from("mangaSectionId"), chapter_id.to_string()));
	params.push((String::from("netType"), String::from("3")));
	params.push((String::from("loadreal"), String::from("1")));
	params.push((String::from("imageQuality"), String::from("2")));

	format!("{}/v1/manga/getRead?{}", API_URL, gen_query_string(params))
}
