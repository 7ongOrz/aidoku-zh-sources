use aidoku::{
	alloc::String,
	helpers::uri::encode_uri,
	imports::{
		defaults::{defaults_get, defaults_set, DefaultValue},
		net::{HttpMethod, Request, Response},
		std::current_date,
	},
	prelude::*,
	AidokuError, Result,
};
use aidoku::alloc::string::ToString;
use md5::compute;
use serde::Deserialize;

use crate::crypto;

const KEY: &[u8; 63] = br"~d}$Q7$eIni=V)9\RK/P.RM4;9[7|@/CA}b~OW!3?EV`:<>M7pddUBL5n|0/*Cn";
const API_KEY: &str = "C69BAF41DA5ABD1FFEDC6D2FEA56B";

const WWW_URL: &str = "https://manhuabika.com";
const API_URL: &str = "https://picaapi.picacomic.com";

#[derive(Deserialize)]
struct LoginResponse {
	data: LoginData,
}

#[derive(Deserialize)]
struct LoginData {
	token: String,
}

pub fn gen_time() -> String {
	(current_date() as i64).to_string()
}

pub fn gen_nonce() -> String {
	format!("{:x}", compute(gen_time()))
}

pub fn gen_signature(url: &str, time: &str, nonce: &str, method: &str) -> String {
	let url = url
		.split_once(&format!("{}/", API_URL))
		.map(|(_, after)| after)
		.unwrap_or(url);
	let text = format!("{}{}{}{}{}", url, time, nonce, method, API_KEY).to_ascii_lowercase();
	crypto::encrypt(text.as_bytes(), KEY)
}

fn method_string(method: HttpMethod) -> &'static str {
	match method {
		HttpMethod::Get => "Get",
		HttpMethod::Post => "Post",
		HttpMethod::Put => "Put",
		HttpMethod::Head => "Head",
		HttpMethod::Delete => "Delete",
		HttpMethod::Patch => "Patch",
		HttpMethod::Options => "Options",
		HttpMethod::Connect => "Connect",
		HttpMethod::Trace => "Trace",
	}
}

pub fn gen_request(url: String, method: HttpMethod) -> Result<Request> {
	let time = gen_time();
	let nonce = gen_nonce();
	let signature = gen_signature(&url, &time, &nonce, method_string(method));
	let token = defaults_get::<String>("token").unwrap_or_default();
	let authorization = if !url.contains("sign-in") && token.is_empty() {
		login()?
	} else {
		token
	};
	Ok(Request::new(&url, method)?
		.header("api-key", API_KEY)
		.header("app-build-version", "45")
		.header("app-channel", "1")
		.header("app-platform", "android")
		.header("app-uuid", "defaultUuid")
		.header("app-version", "2.2.1.3.3.4")
		.header("image-quality", "original")
		.header("time", &time)
		.header("nonce", &nonce)
		.header("signature", &signature)
		.header("Accept", "application/vnd.picacomic.com.v1+json")
		.header("Authorization", &authorization)
		.header("Content-Type", "application/json; charset=UTF-8")
		.header("User-Agent", "okhttp/3.8.1"))
}

pub fn login() -> Result<String> {
	let request = gen_request(gen_login_url(), HttpMethod::Post)?.header("Authorization", "");
	let username = defaults_get::<String>("username").unwrap_or_default();
	let password = defaults_get::<String>("password").unwrap_or_default();

	if username.is_empty() || password.is_empty() {
		return Err(AidokuError::Message("账号或密码未设置".into()));
	}

	let body = format!(
		r#"{{"email": "{}", "password": "{}"}}"#,
		username, password
	);
	let response: Response = request.body(body.as_bytes()).send()?;

	if response.status_code() != 200 {
		return Err(AidokuError::Message("登录失败".into()));
	}

	let login: LoginResponse = response.get_json_owned()?;
	defaults_set("token", DefaultValue::String(login.data.token.clone()));
	Ok(login.data.token)
}

pub fn get_json<T: serde::de::DeserializeOwned>(url: String) -> Result<T> {
	let request = gen_request(url.clone(), HttpMethod::Get)?;
	let response = request.send()?;
	if response.status_code() == 401 {
		let token = login()?;
		let retry = gen_request(url, HttpMethod::Get)?.header("Authorization", &token);
		retry.json_owned()
	} else {
		response.get_json_owned()
	}
}

pub fn search<T: serde::de::DeserializeOwned>(keyword: String, page: i32) -> Result<T> {
	let url = gen_search_url(page);
	let body = format!(r#"{{"keyword": "{}", "sort": "dd"}}"#, keyword);
	let request = gen_request(url.clone(), HttpMethod::Post)?.body(body.as_bytes());
	let response = request.send()?;

	if response.status_code() == 401 {
		let token = login()?;
		let retry = gen_request(url, HttpMethod::Post)?
			.header("Authorization", &token)
			.body(body.as_bytes());
		retry.json_owned()
	} else {
		response.get_json_owned()
	}
}

pub fn gen_login_url() -> String {
	format!("{}/auth/sign-in", API_URL)
}

pub fn gen_explore_url(category: String, sort: String, page: i32) -> String {
	if category.is_empty() {
		format!("{}/comics?page={}&s={}", API_URL, page, sort)
	} else {
		format!(
			"{}/comics?page={}&c={}&s={}",
			API_URL,
			page,
			encode_uri(category),
			sort,
		)
	}
}

pub fn gen_rank_url(time: String) -> String {
	format!("{}/comics/leaderboard?tt={}&ct=VC", API_URL, time)
}

pub fn gen_random_url() -> String {
	format!("{}/comics/random", API_URL)
}

pub fn gen_search_url(page: i32) -> String {
	format!("{}/comics/advanced-search?page={}&s=dd", API_URL, page)
}

pub fn gen_manga_url(id: String) -> String {
	format!("{}/pcomicview/?cid={}", WWW_URL, id)
}

pub fn gen_manga_details_url(id: String) -> String {
	format!("{}/comics/{}", API_URL, id)
}

pub fn gen_chapter_list_url(id: String, page: i32) -> String {
	format!("{}/comics/{}/eps?page={}", API_URL, id, page)
}

pub fn gen_chapter_url(manga_id: String, chapter_id: String) -> String {
	format!(
		"{}/pchapter/?cid={}&chapter={}",
		WWW_URL, manga_id, chapter_id
	)
}

pub fn gen_page_list_url(manga_id: String, chapter_id: String, page: i32) -> String {
	format!(
		"{}/comics/{}/order/{}/pages?page={}",
		API_URL, manga_id, chapter_id, page
	)
}
