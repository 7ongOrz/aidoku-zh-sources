#![no_std]
use aidoku::{
	alloc::{String, Vec},
	helpers::uri::encode_uri,
	imports::net::Request,
	prelude::*,
	Chapter, ContentRating, FilterValue, ImageRequestProvider, Listing, ListingProvider, Manga,
	MangaPageResult, MangaStatus, Page, PageContent, Result, Source, Viewer,
};
use aidoku::alloc::string::ToString;

mod helper;

use helper::{BookItem, BASE_URL, UA};

struct YandansheSource;

fn build_body(page: i32, key: &str, paixu: &str, status: &str, category: &str) -> String {
	let mut body = format!(
		"page={}&key={}&paixu={}&status={}&limitStatus=0",
		page,
		encode_uri(key),
		paixu,
		status
	);
	if category.is_empty() {
		body.push_str("&sort=");
	} else {
		body.push_str("&sort%5B%5D=");
		body.push_str(&encode_uri(category));
	}
	body
}

fn parse_listing(items: &[BookItem]) -> MangaPageResult {
	let entries: Vec<Manga> = items
		.iter()
		.map(|item| Manga {
			key: item.id.clone(),
			title: item.title.clone(),
			cover: Some(item.cover_pic.clone()),
			..Default::default()
		})
		.collect();
	MangaPageResult {
		has_next_page: !entries.is_empty(),
		entries,
	}
}

impl Source for YandansheSource {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut status = String::new();
		let mut category = String::new();

		for filter in filters {
			if let FilterValue::Select { id, value } = filter {
				match id.as_str() {
					"status" => status = value,
					"category" => category = value,
					_ => {}
				}
			}
		}

		let key = query.as_deref().unwrap_or("");
		let paixu = if key.is_empty() { "3" } else { "" };
		let body = build_body(page, key, paixu, &status, &category);
		let items = helper::search_manga(&body)?;
		Ok(parse_listing(&items))
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			let url = format!("{}/home/book/index/id/{}/", BASE_URL, manga.key);
			let html = helper::get_html(&url)?;

			manga.title = html
				.select_first(".cover-box .container .title")
				.and_then(|e| e.text())
				.unwrap_or_default();
			manga.cover = html
				.select_first("meta[property=\"og:image\"]")
				.and_then(|e| e.attr("content"));
			manga.description = html
				.select_first("#book-info .article .body")
				.and_then(|e| e.text())
				.map(|s| s.trim().to_string());
			manga.authors = html
				.select_first("#book-info .article .author")
				.and_then(|e| e.text())
				.map(|s| {
					s.replace("作者：", "")
						.split(',')
						.map(|a| a.trim().to_string())
						.collect::<Vec<String>>()
				});

			let status_text = html
				.select_first("#chapters .ch .title")
				.and_then(|e| e.text())
				.unwrap_or_default();
			manga.status = if status_text.contains("已完结") {
				MangaStatus::Completed
			} else if status_text.contains("连载") || status_text.contains("更新") {
				MangaStatus::Ongoing
			} else {
				MangaStatus::Unknown
			};

			manga.content_rating = ContentRating::NSFW;
			manga.viewer = Viewer::Webtoon;
			manga.url = Some(url);
			manga.tags = html.select(".cover-box .tags .label .item a").map(|items| {
				items.filter_map(|a| a.text()).collect::<Vec<String>>()
			});
		}

		if needs_chapters {
			manga.chapters = Some(helper::get_all_chapters(&manga.key)?);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{}/home/book/inforedit/{}", BASE_URL, chapter.key);
		let text = Request::get(&url)?
			.header("User-Agent", UA)
			.string()?;
		let urls = helper::decrypt_images(&text)?;
		Ok(urls
			.into_iter()
			.map(|u| Page {
				content: PageContent::url(u),
				..Default::default()
			})
			.collect())
	}
}

impl ListingProvider for YandansheSource {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let paixu = match listing.id.as_str() {
			"更新" => "3",
			"新作" => "2",
			"人气" => "1",
			_ => return self.get_search_manga_list(None, page, Vec::new()),
		};
		let body = build_body(page, "", paixu, "", "");
		let items = helper::search_manga(&body)?;
		Ok(parse_listing(&items))
	}
}

impl ImageRequestProvider for YandansheSource {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		// Image URL patterns:
		//   Cover:   img.yidanmh.xyz/bookimage/{book_id}/{file}
		//   Chapter: img.yidanmh.xyz/bookimage/{book_id}/{chapter_id}/{file}
		let referer = if let Some(pos) = url.find("/bookimage/") {
			let parts: Vec<&str> = url[pos + 11..].split('/').collect();
			if parts.len() >= 3 {
				format!(
					"{}/home/book/inforedit/{}/{}",
					BASE_URL, parts[0], parts[1]
				)
			} else if !parts.is_empty() {
				format!("{}/home/book/index/id/{}/", BASE_URL, parts[0])
			} else {
				format!("{}/", BASE_URL)
			}
		} else {
			format!("{}/", BASE_URL)
		};

		Ok(Request::get(&url)?
			.header("User-Agent", UA)
			.header("Referer", &referer))
	}
}

register_source!(YandansheSource, ListingProvider, ImageRequestProvider);
