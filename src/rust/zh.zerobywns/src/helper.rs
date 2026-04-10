use aidoku::{
	alloc::{String, Vec},
	imports::{
		defaults::{defaults_get, defaults_set, DefaultValue},
		html::Document,
		net::{HttpMethod, Request},
	},
	prelude::format,
	Result,
};
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36";

fn handle_cookie_header(cookie_header: &str) -> String {
	cookie_header
		.replace(",", ";")
		.split(";")
		.filter(|a| a.contains("Ckng"))
		.map(|a| a.trim())
		.collect::<Vec<&str>>()
		.join(";")
}

pub fn get_url() -> String {
	defaults_get::<String>("url").unwrap_or_default()
}

pub fn get_html(url: &str) -> Result<Document> {
	let default_cookie: String = defaults_get("cookie").unwrap_or_default();
	let response = Request::get(url)?
		.header("User-Agent", UA)
		.header("Cookie", &default_cookie)
		.send()?;

	let cookie_header = response.get_header("set-cookie").unwrap_or_default();
	let html = response.get_html()?;

	let needs_login = html
		.select_first("#main_message #messagetext>p")
		.and_then(|e| e.text())
		.unwrap_or_default()
		.contains("仅限用户观看，请先登录");

	if needs_login {
		let username: String = defaults_get("username").unwrap_or_default();
		let password: String = defaults_get("password").unwrap_or_default();

		if username.is_empty() || password.is_empty() {
			return Ok(html);
		}

		let formhash = html
			.select_first("input[name=formhash]")
			.and_then(|e| e.attr("value"))
			.unwrap_or_default();
		let login_cookie = handle_cookie_header(&cookie_header);
		let body = format!(
			"username={}&cookietime=2592000&password={}&formhash={}&quickforward=yes&handlekey=ls",
			username, password, formhash
		);
		let base_url = get_url();
		let login_url = format!(
			"{}/member.php?mod=logging&action=login&loginsubmit=yes&infloat=yes&lssubmit=yes&inajax=1",
			base_url
		);
		let login_response = Request::new(&login_url, HttpMethod::Post)?
			.header("User-Agent", UA)
			.header("Content-Type", "application/x-www-form-urlencoded")
			.header("Cookie", &login_cookie)
			.body(body.as_bytes())
			.send()?;

		let new_cookie_header = login_response
			.get_header("set-cookie")
			.unwrap_or_default();

		if new_cookie_header.contains("auth") {
			let new_cookie = handle_cookie_header(&new_cookie_header);
			defaults_set("cookie", DefaultValue::String(new_cookie));
			return get_html(url);
		}
	}

	Ok(html)
}
