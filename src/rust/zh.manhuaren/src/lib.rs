#![no_std]
use aidoku::{
	alloc::{String, Vec},
	imports::net::Request,
	prelude::*,
	Chapter, FilterValue, ImageRequestProvider, Listing, ListingProvider, Manga, MangaPageResult,
	Page, Result, Source,
};
use aidoku::alloc::string::ToString;

mod helper;
mod parser;

use parser::ListingResponse;

const FILTER_SORT: [&str; 3] = ["10", "2", "18"];

struct ManhuarenSource;

impl Source for ManhuarenSource {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		if let Some(ref q) = query {
			let html = helper::search_html(q)?;
			let entries = parser::parse_search(&html);
			Ok(MangaPageResult {
				has_next_page: false,
				entries,
			})
		} else {
			let mut tagid = String::from("0");
			let mut status = String::from("0");
			let mut sort = String::from("10");

			for filter in filters {
				match filter {
					FilterValue::Select { id, value } => match id.as_str() {
						"category" => tagid = value,
						"status" => status = value,
						_ => {}
					},
					FilterValue::Sort { id, index, .. } => {
						if id == "sort" {
							sort = FILTER_SORT
								.get(index as usize)
								.unwrap_or(&"10")
								.to_string();
						}
					}
					_ => {}
				}
			}

			let body = format!(
				"action=getclasscomics&pageindex={}&pagesize=21&categoryid=0&tagid={}&status={}&usergroup=0&pay=-1&areaid=0&sort={}&iscopyright=0",
				page, tagid, status, sort
			);
			let resp: ListingResponse = helper::post_json(&body)?;
			let entries = parser::parse_listing(&resp.items);
			Ok(MangaPageResult {
				has_next_page: !entries.is_empty(),
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
		let url = helper::manga_url(&manga.key);
		let html = helper::get_html(&url)?;

		if needs_details {
			let detail = parser::parse_manga_detail(&html);
			manga.title = detail.title;
			manga.cover = detail.cover;
			manga.authors = detail.authors;
			manga.description = detail.description;
			manga.url = Some(url.clone());
			manga.tags = detail.tags;
			manga.status = detail.status;
			manga.content_rating = detail.content_rating;
			manga.viewer = detail.viewer;
		}

		if needs_chapters {
			manga.chapters = Some(parser::parse_chapter_list(&html));
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = helper::chapter_url(&chapter.key);
		let text = helper::get_text(&url)?;
		Ok(parser::parse_page_list(&text))
	}
}

impl ListingProvider for ManhuarenSource {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let sort = match listing.id.as_str() {
			"最热门" => "10",
			"最近更新" => "2",
			"最新上架" => "18",
			_ => return self.get_search_manga_list(None, page, Vec::new()),
		};
		let body = format!(
			"action=getclasscomics&pageindex={}&pagesize=21&categoryid=0&tagid=0&status=0&usergroup=0&pay=-1&areaid=0&sort={}&iscopyright=0",
			page, sort
		);
		let resp: ListingResponse = helper::post_json(&body)?;
		let entries = parser::parse_listing(&resp.items);
		Ok(MangaPageResult {
			has_next_page: !entries.is_empty(),
			entries,
		})
	}
}

impl ImageRequestProvider for ManhuarenSource {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		let referer = url
			.find("cid=")
			.map(|pos| {
				let start = pos + 4;
				let end = url[start..]
					.find('&')
					.map(|i| start + i)
					.unwrap_or(url.len());
				format!("https://www.manhuaren.com/m{}/", &url[start..end])
			})
			.unwrap_or_else(|| String::from("https://www.manhuaren.com/"));

		Ok(Request::get(&url)?
			.header("User-Agent", helper::UA)
			.header("Referer", &referer))
	}
}

register_source!(ManhuarenSource, ListingProvider, ImageRequestProvider);
