use aidoku::{
	alloc::String,
	helpers::uri::{encode_uri, encode_uri_component},
	imports::{
		defaults::{defaults_get, defaults_set, DefaultValue},
		net::{HttpMethod, Request},
		std::current_date,
	},
	prelude::*,
	AidokuError, Result,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::Value;

pub const API_URL: &str = "https://api.2025copy.com/api/v3";
pub const WWW_URL: &str = "https://www.2025copy.com";

const USER_AGENT: &str = "COPY/3.0.0";
const VERSION: &str = "2025.08.15";
const PLATFORM: &str = "1";

const KEY_TOKEN: &str = "copymanga_token";
const KEY_TOKEN_OWNER: &str = "copymanga_token_owner";
const KEY_ANON_USERNAME: &str = "copymanga_anon_username";
const KEY_ANON_PASSWORD: &str = "copymanga_anon_password";

#[derive(Deserialize)]
struct CopyResp {
	code: i32,
	#[serde(default)]
	message: String,
	#[serde(default)]
	results: Value,
}

#[derive(Deserialize)]
struct LoginResults {
	token: String,
}

fn new_get(url: &str) -> Result<Request> {
	Ok(Request::get(url)?
		.header("User-Agent", USER_AGENT)
		.header("Accept", "application/json")
		.header("version", VERSION)
		.header("platform", PLATFORM)
		.header("webp", "1")
		.header("region", "1"))
}

fn new_post(url: &str) -> Result<Request> {
	Ok(Request::new(url, HttpMethod::Post)?
		.header("User-Agent", USER_AGENT)
		.header("Accept", "application/json")
		.header("version", VERSION)
		.header("platform", PLATFORM)
		.header("webp", "1")
		.header("region", "1")
		.header("Content-Type", "application/x-www-form-urlencoded"))
}

fn parse_resp<T: DeserializeOwned>(data: &[u8]) -> Result<(i32, String, Option<T>)> {
	let resp: CopyResp = serde_json::from_slice(data)?;
	let results = if resp.code == 200 {
		Some(serde_json::from_value::<T>(resp.results)?)
	} else {
		None
	};
	Ok((resp.code, resp.message, results))
}

/// 匿名公开接口（/comics /ranks /search/comic 等）
pub fn get_json<T: DeserializeOwned>(url: &str) -> Result<T> {
	let data = new_get(url)?.data()?;
	let (code, message, results) = parse_resp::<T>(&data)?;
	if code == 200 {
		results.ok_or_else(|| AidokuError::message("API 返回 200 但无结果"))
	} else {
		Err(AidokuError::message(format!(
			"API 错误 {}: {}",
			code, message
		)))
	}
}

/// 需要鉴权的接口（/comic2 /group/{g}/chapters /chapter2 等），遇到 210/401 自动重试一次
pub fn get_json_authed<T: DeserializeOwned>(url: &str) -> Result<T> {
	let token = ensure_token()?;
	let authorization = format!("Token {}", token);
	let data = new_get(url)?
		.header("authorization", authorization.as_str())
		.data()?;
	let (code, message, results) = parse_resp::<T>(&data)?;

	if code == 200 {
		return results.ok_or_else(|| AidokuError::message("API 返回 200 但无结果"));
	}

	if code == 210 || code == 401 {
		clear_auth();
		let token = ensure_token()?;
		let authorization = format!("Token {}", token);
		let data = new_get(url)?
			.header("authorization", authorization.as_str())
			.data()?;
		let (code, message, results) = parse_resp::<T>(&data)?;
		if code == 200 {
			return results.ok_or_else(|| AidokuError::message("API 返回 200 但无结果"));
		}
		return Err(AidokuError::message(format!(
			"鉴权后仍失败 {}: {}",
			code, message
		)));
	}

	Err(AidokuError::message(format!(
		"API 错误 {}: {}",
		code, message
	)))
}

fn clear_token() {
	defaults_set(KEY_TOKEN, DefaultValue::String(String::new()));
}

fn clear_anon_account() {
	defaults_set(KEY_ANON_USERNAME, DefaultValue::String(String::new()));
	defaults_set(KEY_ANON_PASSWORD, DefaultValue::String(String::new()));
}

/// 鉴权失败时调用：清 token，若当前用的是匿名账号则一并清掉（让 ensure_token 注册新的）
fn clear_auth() {
	clear_token();
	let user_name = defaults_get::<String>("username").unwrap_or_default();
	let user_pass = defaults_get::<String>("password").unwrap_or_default();
	if user_name.is_empty() || user_pass.is_empty() {
		clear_anon_account();
	}
}

/// 确保有可用 token：优先 settings 账号 → 持久化的匿名账号 → 新注册匿名账号
pub fn ensure_token() -> Result<String> {
	let user_name = defaults_get::<String>("username").unwrap_or_default();
	let user_pass = defaults_get::<String>("password").unwrap_or_default();
	let has_user_account = !user_name.is_empty() && !user_pass.is_empty();

	// 账号变更检测：settings username 和 token 来源不一致时清 token
	let owner = defaults_get::<String>(KEY_TOKEN_OWNER).unwrap_or_default();
	if owner != user_name {
		clear_token();
	}

	if let Some(cached) = defaults_get::<String>(KEY_TOKEN) {
		if !cached.is_empty() {
			return Ok(cached);
		}
	}

	if has_user_account {
		let token = login(&user_name, &user_pass)?;
		defaults_set(KEY_TOKEN, DefaultValue::String(token.clone()));
		defaults_set(KEY_TOKEN_OWNER, DefaultValue::String(user_name));
		return Ok(token);
	}

	// 复用之前注册的匿名账号
	let anon_name = defaults_get::<String>(KEY_ANON_USERNAME).unwrap_or_default();
	let anon_pass = defaults_get::<String>(KEY_ANON_PASSWORD).unwrap_or_default();
	if !anon_name.is_empty() && !anon_pass.is_empty() {
		if let Ok(token) = login(&anon_name, &anon_pass) {
			defaults_set(KEY_TOKEN, DefaultValue::String(token.clone()));
			defaults_set(KEY_TOKEN_OWNER, DefaultValue::String(String::new()));
			return Ok(token);
		}
	}

	// 注册新匿名账号
	let (username, password) = gen_anon_account();
	register(&username, &password)?;
	let token = login(&username, &password)?;
	defaults_set(KEY_ANON_USERNAME, DefaultValue::String(username));
	defaults_set(KEY_ANON_PASSWORD, DefaultValue::String(password));
	defaults_set(KEY_TOKEN, DefaultValue::String(token.clone()));
	defaults_set(KEY_TOKEN_OWNER, DefaultValue::String(String::new()));
	Ok(token)
}

fn gen_anon_account() -> (String, String) {
	let ts = current_date() as u64;
	let spread = ts.wrapping_mul(0x9E37_79B9_7F4A_7C15);
	let username = format!("Aidoku{}", to_base36(ts));
	let password = format!("Pwd{}Ai9", to_base36(spread));
	(username, password)
}

fn to_base36(mut n: u64) -> String {
	if n == 0 {
		return "0".into();
	}
	let mut buf = [0u8; 13];
	let mut i = buf.len();
	while n > 0 {
		i -= 1;
		let r = (n % 36) as u8;
		buf[i] = if r < 10 { b'0' + r } else { b'a' + (r - 10) };
		n /= 36;
	}
	String::from_utf8(buf[i..].to_vec()).unwrap_or_default()
}

fn register(username: &str, password: &str) -> Result<()> {
	let url = format!("{}/register", API_URL);
	let body = format!(
		"username={}&password={}&source=freeSite",
		encode_uri_component(username),
		encode_uri_component(password),
	);
	let data = new_post(&url)?.body(body.as_bytes()).data()?;
	let resp: CopyResp = serde_json::from_slice(&data)?;
	if resp.code != 200 {
		return Err(AidokuError::message(format!(
			"注册失败 {}: {}",
			resp.code, resp.message
		)));
	}
	Ok(())
}

fn login(username: &str, password: &str) -> Result<String> {
	let encoded_pwd = STANDARD.encode(format!("{}-1729", password).as_bytes());
	let url = format!("{}/login", API_URL);
	let body = format!(
		"username={}&password={}&salt=1729",
		encode_uri_component(username),
		encode_uri_component(&encoded_pwd),
	);
	let data = new_post(&url)?.body(body.as_bytes()).data()?;
	let (code, message, results) = parse_resp::<LoginResults>(&data)?;
	if code == 200 {
		results
			.map(|r| r.token)
			.ok_or_else(|| AidokuError::message("登录响应缺少 token"))
	} else {
		Err(AidokuError::message(format!(
			"登录失败 {}: {}",
			code, message
		)))
	}
}

// ---------- URL 构造 ----------

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
		"{}/search/comic?q={}&q_type=&limit=20&offset={}&platform=1",
		API_URL,
		encode_uri(query),
		(page - 1) * 20,
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

pub fn gen_comic_url(path_word: &str) -> String {
	format!("{}/comic2/{}?platform=1", API_URL, path_word)
}

pub fn gen_chapters_url(path_word: &str, group: &str, limit: i32, offset: i32) -> String {
	format!(
		"{}/comic/{}/group/{}/chapters?limit={}&offset={}",
		API_URL, path_word, group, limit, offset,
	)
}

pub fn gen_chapter_detail_url(path_word: &str, chapter_uuid: &str) -> String {
	format!(
		"{}/comic/{}/chapter2/{}?platform=1",
		API_URL, path_word, chapter_uuid,
	)
}

pub fn gen_web_manga_url(path_word: &str) -> String {
	format!("{}/comic/{}", WWW_URL, path_word)
}

pub fn gen_web_chapter_url(path_word: &str, chapter_uuid: &str) -> String {
	format!("{}/comic/{}/chapter/{}", WWW_URL, path_word, chapter_uuid)
}
