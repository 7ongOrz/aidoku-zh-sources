#![no_std]
use aidoku::{
	alloc::{String, Vec},
	imports::net::Request,
	prelude::*,
	Chapter, FilterValue, ImageRequestProvider, Manga, MangaPageResult, Page, Result, Source,
};
use aidoku::alloc::string::ToString;

mod helper;
mod parser;

use parser::{ApiResponse, PageResponse};

const FILTER_SORT: [&str; 3] = ["0", "1", "2"];

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
		let mut category = String::from("0");
		let mut status = String::from("0");
		let mut sort = String::from("0");

		for filter in filters {
			match filter {
				FilterValue::Select { id, value } => match id.as_str() {
					"category" => category = value,
					"status" => status = value,
					_ => {}
				},
				FilterValue::Sort { id, index, .. } => {
					if id == "sort" {
						sort = FILTER_SORT
							.get(index as usize)
							.unwrap_or(&"0")
							.to_string();
					}
				}
				_ => {}
			}
		}

		if let Some(ref q) = query {
			let url = helper::gen_search_url(q, page);
			let resp: ApiResponse = helper::get_json(&url)?;
			let entries = parser::parse_manga_list(&resp.response.result);
			let has_next_page = page * 20 < resp.response.total;
			Ok(MangaPageResult {
				has_next_page,
				entries,
			})
		} else {
			let url = helper::gen_explore_url(&category, &status, &sort, page);
			let resp: ApiResponse = helper::get_json(&url)?;
			let entries = parser::parse_manga_list(&resp.response.mangas);
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
		let url = helper::gen_manga_details_url(&manga.key);
		let resp: ApiResponse = helper::get_json(&url)?;

		if needs_details {
			let detailed = parser::parse_manga_detail(&resp.response);
			manga.title = detailed.title;
			manga.cover = detailed.cover;
			manga.authors = detailed.authors;
			manga.description = detailed.description;
			manga.url = detailed.url;
			manga.tags = detailed.tags;
			manga.status = detailed.status;
			manga.content_rating = detailed.content_rating;
			manga.viewer = detailed.viewer;
		}

		if needs_chapters {
			manga.chapters = Some(parser::parse_chapter_list(&resp.response));
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = helper::gen_page_list_url(&manga.key, &chapter.key);
		let resp: PageResponse = helper::get_json(&url)?;
		Ok(parser::parse_page_list(&resp.response))
	}
}

impl ImageRequestProvider for ManhuarenSource {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		Ok(Request::get(&url)?
			.header("X-Yq-Yqci", r#"{"le": "zh"}"#)
			.header("User-Agent", "okhttp/3.11.0")
			.header("Referer", "http://www.dm5.com/dm5api/")
			.header("ClubReferer", "http://mangaapi.manhuaren.com/"))
	}
}

register_source!(ManhuarenSource, ImageRequestProvider);
