#![no_std]
use aidoku::{
	alloc::{String, Vec},
	prelude::*,
	Chapter, ContentRating, FilterValue, Listing, ListingProvider, Manga, MangaPageResult,
	MangaStatus, Page, Result, Source, Viewer,
};
use aidoku::alloc::string::ToString;

mod crypto;
mod helper;
mod parser;

const FILTER_ORDERING: [&str; 2] = ["popular", "datetime_updated"];

struct CopymangaSource;

impl Source for CopymangaSource {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut theme = String::new();
		let mut top = String::new();
		let mut ordering = String::from("-popular");

		for filter in filters {
			match filter {
				FilterValue::Select { id, value } => match id.as_str() {
					"theme" => theme = value,
					"country" => top = value,
					_ => {}
				},
				FilterValue::Sort { id, index, ascending } => {
					if id == "sort" {
						let mut s = String::new();
						if !ascending {
							s.push('-');
						}
						if let Some(v) = FILTER_ORDERING.get(index as usize) {
							s.push_str(v);
						}
						ordering = s;
					}
				}
				_ => {}
			}
		}

		let url = if let Some(q) = query {
			helper::gen_search_url(&q, page)
		} else {
			helper::gen_explore_url(&theme, &top, &ordering, page)
		};

		let resp: parser::ListResponse = helper::get_json(&url)?;
		let has_next_page = resp.results.total > resp.results.limit + resp.results.offset;
		Ok(MangaPageResult {
			entries: parser::parse_manga_list(resp.results.list),
			has_next_page,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			let url = helper::gen_manga_url(&manga.key);
			let html = helper::get_html(&url)?;

			manga.cover = html
				.select_first(".comicParticulars-left-img>img")
				.and_then(|e| e.attr("data-src"));
			manga.title = html
				.select_first("h6")
				.and_then(|e| e.text())
				.unwrap_or_default();

			let mut authors = Vec::new();
			if let Some(items) = html.select(".comicParticulars-right-txt>a") {
				for item in items {
					if let Some(t) = item.text() {
						authors.push(t);
					}
				}
			}
			manga.authors = Some(authors);

			manga.description = html
				.select_first(".intro")
				.and_then(|e| e.text())
				.map(|s| s.trim().to_string());

			let mut tags = Vec::new();
			if let Some(items) = html.select(".comicParticulars-tag>a") {
				for item in items {
					if let Some(t) = item.text() {
						tags.push(t.replace('#', ""));
					}
				}
			}
			manga.tags = Some(tags);

			let full_title = html
				.select_first("title")
				.and_then(|e| e.text())
				.unwrap_or_default();
			manga.status = if full_title.contains("連載中") {
				MangaStatus::Ongoing
			} else if full_title.contains("已完結") {
				MangaStatus::Completed
			} else {
				MangaStatus::Unknown
			};
			manga.content_rating = ContentRating::Safe;
			manga.viewer = Viewer::RightToLeft;
			manga.url = Some(url);
		}

		if needs_chapters {
			let manga_url = helper::gen_manga_url(&manga.key);
			let text = helper::get_text(&manga_url)?;
			let key = text
				.split_once("var ccx = '")
				.and_then(|(_, after)| after.split_once('\''))
				.map(|(before, _)| before.to_string())
				.unwrap_or_default();
			let url = helper::gen_chapter_list_url(&manga.key);
			let resp: parser::EncryptedResponse = helper::get_json(&url)?;
			let decrypted = helper::decrypt(resp.results, key);
			let data: parser::ChapterRoot = serde_json::from_str(&decrypted)?;
			manga.chapters = Some(parser::parse_chapter_list(data));
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = helper::gen_page_list_url(&manga.key, &chapter.key);
		let text = helper::get_text(&url)?;
		let key = text
			.split_once("var ccy = '")
			.and_then(|(_, after)| after.split_once('\''))
			.map(|(before, _)| before.to_string())
			.unwrap_or_default();
		let data = text
			.split_once("contentKey=\"")
			.and_then(|(_, after)| after.split_once('"'))
			.map(|(before, _)| before.to_string())
			.unwrap_or_default();
		let decrypted = helper::decrypt(data, key);
		let pages: Vec<parser::PageItem> = serde_json::from_str(&decrypted)?;
		Ok(parser::parse_page_list(pages))
	}
}

impl ListingProvider for CopymangaSource {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let url = match listing.id.as_str() {
			"日榜" => helper::gen_rank_url("day", page),
			"周榜" => helper::gen_rank_url("week", page),
			"月榜" => helper::gen_rank_url("month", page),
			"总榜" => helper::gen_rank_url("total", page),
			"编辑推荐" => helper::gen_recs_url(page),
			"全新上架" => helper::gen_newest_url(page),
			_ => return self.get_search_manga_list(None, page, Vec::new()),
		};
		let resp: parser::ListResponse = helper::get_json(&url)?;
		let has_next_page = resp.results.total > resp.results.limit + resp.results.offset;
		Ok(MangaPageResult {
			entries: parser::parse_manga_list(resp.results.list),
			has_next_page,
		})
	}
}

register_source!(CopymangaSource, ListingProvider);
