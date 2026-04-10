#![no_std]
use aidoku::{
	alloc::{String, Vec},
	imports::{
		defaults::{defaults_get, defaults_set, DefaultValue},
		net::{HttpMethod, Request},
	},
	prelude::*,
	Chapter, ContentRating, FilterValue, ImageRequestProvider, Listing, ListingProvider, Manga,
	MangaPageResult, MangaStatus, Page, PageContent, Result, Source, Viewer,
};
use aidoku::alloc::string::ToString;
use serde::Deserialize;

const WWW_URL: &str = "https://noy1.top";
const PIC_URL: &str = "https://img.noy.asia";

const FILTER_SORT: [&str; 3] = ["bid", "views", "favorites"];

#[derive(Deserialize)]
struct ListResponse {
	info: Option<Vec<MangaItem>>,
	#[serde(rename = "Info")]
	info_upper: Option<Vec<MangaItem>>,
	len: i64,
}

#[derive(Deserialize)]
struct MangaItem {
	#[serde(rename = "Bid")]
	bid: String,
	#[serde(rename = "Bookname")]
	bookname: String,
	#[serde(rename = "Author")]
	author: String,
	#[serde(rename = "Ptag")]
	ptag: String,
}

#[derive(Deserialize)]
struct DetailResponse {
	#[serde(rename = "Bookname")]
	bookname: String,
	#[serde(rename = "Author")]
	author: String,
	#[serde(rename = "Ptag")]
	ptag: String,
	#[serde(rename = "Len")]
	len: i32,
}

fn get_session() -> String {
	defaults_get::<String>("session").unwrap_or_default()
}

fn login() -> Result<String> {
	let username: String = defaults_get("username").unwrap_or_default();
	let password: String = defaults_get("password").unwrap_or_default();

	let url = format!("{}/api/login", WWW_URL);
	let body = format!("user={}&pass={}", username, password);
	let resp = Request::new(&url, HttpMethod::Post)?
		.header("Content-Type", "application/x-www-form-urlencoded")
		.body(body.as_bytes())
		.send()?;

	let cookie_header = resp.get_header("set-cookie").unwrap_or_default();
	let session = cookie_header
		.split_once("NOY_SESSION=")
		.and_then(|(_, after)| after.split_once(";"))
		.map(|(before, _)| before.to_string())
		.unwrap_or_default();

	defaults_set("session", DefaultValue::String(session.clone()));
	Ok(session)
}

fn post_json<T: serde::de::DeserializeOwned>(url: &str, body: &str) -> Result<T> {
	let session = get_session();
	let session = if session.is_empty() {
		login()?
	} else {
		session
	};

	let cookie = format!("NOY_SESSION={}", session);
	let resp = Request::new(url, HttpMethod::Post)?
		.header("Content-Type", "application/x-www-form-urlencoded")
		.header("Cookie", &cookie)
		.body(body.as_bytes())
		.send()?;

	if resp.status_code() == 401 {
		let new_session = login()?;
		let cookie = format!("NOY_SESSION={}", new_session);
		return Request::new(url, HttpMethod::Post)?
			.header("Content-Type", "application/x-www-form-urlencoded")
			.header("Cookie", &cookie)
			.body(body.as_bytes())
			.json_owned();
	}

	let value: T = resp.get_json_owned()?;
	Ok(value)
}

fn parse_manga_item(item: &MangaItem) -> Manga {
	let tags: Vec<String> = item
		.ptag
		.split(' ')
		.filter(|s| !s.is_empty())
		.map(|s| s.to_string())
		.collect();
	Manga {
		key: item.bid.clone(),
		title: item.bookname.clone(),
		cover: Some(format!("{}/{}/m1.webp", PIC_URL, item.bid)),
		authors: Some(aidoku::alloc::vec![item.author.clone()]),
		tags: Some(tags),
		url: Some(format!("{}/#/book/{}", WWW_URL, item.bid)),
		status: MangaStatus::Completed,
		content_rating: ContentRating::NSFW,
		viewer: Viewer::RightToLeft,
		..Default::default()
	}
}

struct Noy1Source;

impl Source for Noy1Source {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut tag = String::new();
		let mut sort = String::from("bid");

		for filter in filters {
			match filter {
				FilterValue::Select { id, value } => {
					if id == "tag" {
						tag = value;
					}
				}
				FilterValue::Sort { id, index, .. } => {
					if id == "sort" {
						if let Some(s) = FILTER_SORT.get(index as usize) {
							sort = s.to_string();
						}
					}
				}
				_ => {}
			}
		}

		if let Some(query) = query {
			let url = format!("{}/api/search_v2", WWW_URL);
			let body = format!("info={}&type=de&sort=bid&page={}", query, page);
			let resp: ListResponse = post_json(&url, &body)?;
			let items = resp.info_upper.or(resp.info).unwrap_or_default();
			let entries: Vec<Manga> = items.iter().map(parse_manga_item).collect();
			let has_next_page = page * 20 < resp.len as i32;
			Ok(MangaPageResult {
				has_next_page,
				entries,
			})
		} else if tag.is_empty() {
			let url = format!("{}/api/booklist_v2", WWW_URL);
			let body = format!("page={}", page);
			let resp: ListResponse = post_json(&url, &body)?;
			let items = resp.info.unwrap_or_default();
			let entries: Vec<Manga> = items.iter().map(parse_manga_item).collect();
			let has_next_page = page * 20 < resp.len as i32;
			Ok(MangaPageResult {
				has_next_page,
				entries,
			})
		} else {
			let url = format!("{}/api/search_v2", WWW_URL);
			let body = format!("info={}&type=tag&sort={}&page={}", tag, sort, page);
			let resp: ListResponse = post_json(&url, &body)?;
			let items = resp.info_upper.or(resp.info).unwrap_or_default();
			let entries: Vec<Manga> = items.iter().map(parse_manga_item).collect();
			let has_next_page = page * 20 < resp.len as i32;
			Ok(MangaPageResult {
				has_next_page,
				entries,
			})
		}
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			let url = format!("{}/api/getbookinfo", WWW_URL);
			let body = format!("bid={}", manga.key);
			let detail: DetailResponse = post_json(&url, &body)?;

			manga.title = detail.bookname;
			manga.cover = Some(format!("{}/{}/m1.webp", PIC_URL, manga.key));
			manga.authors = Some(aidoku::alloc::vec![detail.author]);
			manga.tags = Some(
				detail
					.ptag
					.split(' ')
					.filter(|s| !s.is_empty())
					.map(|s| s.to_string())
					.collect(),
			);
			manga.url = Some(format!("{}/#/book/{}", WWW_URL, manga.key));
			manga.status = MangaStatus::Completed;
			manga.content_rating = ContentRating::NSFW;
			manga.viewer = Viewer::RightToLeft;
		}

		if needs_chapters {
			manga.chapters = Some(aidoku::alloc::vec![Chapter {
				key: manga.key.clone(),
				title: Some(String::from("第 1 话")),
				chapter_number: Some(1.0),
				url: Some(format!("{}/#/read/{}", WWW_URL, manga.key)),
				..Default::default()
			}]);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, _chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{}/api/getbookinfo", WWW_URL);
		let body = format!("bid={}", manga.key);
		let detail: DetailResponse = post_json(&url, &body)?;

		let pages: Vec<Page> = (1..=detail.len)
			.map(|i| Page {
				content: PageContent::url(format!("{}/{}/{}.webp", PIC_URL, manga.key, i)),
				..Default::default()
			})
			.collect();
		Ok(pages)
	}
}

impl ListingProvider for Noy1Source {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let (name, level) = match listing.id.as_str() {
			"日阅读榜" => ("readLeaderboard", "day"),
			"周阅读榜" => ("readLeaderboard", "week"),
			"月阅读榜" => ("readLeaderboard", "moon"),
			"日收藏榜" => ("favLeaderboard", "day"),
			"周收藏榜" => ("favLeaderboard", "week"),
			"月收藏榜" => ("favLeaderboard", "moon"),
			"高质量榜" => ("proportion", ""),
			_ => return self.get_search_manga_list(None, page, Vec::new()),
		};

		let url = format!("{}/api/{}", WWW_URL, name);
		let body = if !level.is_empty() {
			format!("page={}&type={}", page, level)
		} else {
			format!("page={}", page)
		};
		let resp: ListResponse = post_json(&url, &body)?;
		let items = resp.info.unwrap_or_default();
		let entries: Vec<Manga> = items.iter().map(parse_manga_item).collect();
		let has_next_page = page * 20 < resp.len as i32;
		Ok(MangaPageResult {
			has_next_page,
			entries,
		})
	}
}

impl ImageRequestProvider for Noy1Source {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		Ok(Request::get(&url)?.header("Referer", WWW_URL))
	}
}

register_source!(Noy1Source, ListingProvider, ImageRequestProvider);
