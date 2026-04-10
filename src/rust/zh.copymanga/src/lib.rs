#![no_std]
use aidoku::{
	alloc::{String, Vec},
	prelude::*,
	Chapter, FilterValue, Listing, ListingProvider, Manga, MangaPageResult, Page, Result, Source,
};

mod helper;
mod parser;

const FILTER_ORDERING: [&str; 2] = ["popular", "datetime_updated"];
const CHAPTERS_PAGE_LIMIT: i32 = 500;

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

		let results: parser::ListResults = helper::get_json(&url)?;
		let has_next_page = results.total > results.limit + results.offset;
		Ok(MangaPageResult {
			entries: parser::parse_manga_list(results.list),
			has_next_page,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let comic_url = helper::gen_comic_url(&manga.key);
		let comic: parser::GetComicResults = helper::get_json_authed(&comic_url)?;

		if needs_details {
			parser::fill_manga_details(&mut manga, comic.comic);
		}

		if needs_chapters {
			let groups = parser::ordered_groups(comic.groups);
			let mut all_chapters: Vec<Chapter> = Vec::new();
			for group in groups {
				let mut offset = 0i32;
				loop {
					let url = helper::gen_chapters_url(
						&manga.key,
						&group.path_word,
						CHAPTERS_PAGE_LIMIT,
						offset,
					);
					let page: parser::GetChaptersResults = helper::get_json_authed(&url)?;
					let got = page.list.len();
					let start = all_chapters.len();
					all_chapters.extend(parser::parse_chapters(
						&manga.key,
						&group.name,
						page.list,
						start,
					));
					offset += got as i32;
					if got == 0 || offset >= page.total {
						break;
					}
				}
			}
			all_chapters.reverse();
			manga.chapters = Some(all_chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = helper::gen_chapter_detail_url(&manga.key, &chapter.key);
		let results: parser::GetChapterResults = helper::get_json_authed(&url)?;
		Ok(parser::parse_page_list(results.chapter))
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
		let results: parser::ListResults = helper::get_json(&url)?;
		let has_next_page = results.total > results.limit + results.offset;
		Ok(MangaPageResult {
			entries: parser::parse_manga_list(results.list),
			has_next_page,
		})
	}
}

register_source!(CopymangaSource, ListingProvider);
