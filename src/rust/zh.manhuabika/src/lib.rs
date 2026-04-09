#![no_std]
use aidoku::{
	alloc::{String, Vec},
	prelude::*,
	Chapter, FilterValue, Listing, ListingProvider, Manga, MangaPageResult, Page, Result, Source,
};
use aidoku::alloc::string::ToString;
use serde::Deserialize;

mod crypto;
mod helper;
mod parser;

use parser::{
	ComicsContainer, EpsResponse, MangaItem, PagedList, PagesResponse, PicaResponse,
};

const FILTER_SORT: [&str; 4] = ["dd", "da", "ld", "vd"];

struct BikaSource;

#[derive(Deserialize)]
struct ComicsListResponse {
	data: ComicsContainer<PagedList<MangaItem>>,
}

#[derive(Deserialize)]
struct RankComicsResponse {
	data: RankComicsData,
}

#[derive(Deserialize)]
struct RankComicsData {
	comics: Vec<MangaItem>,
}

#[derive(Deserialize)]
struct SingleMangaResponse {
	data: SingleMangaData,
}

#[derive(Deserialize)]
struct SingleMangaData {
	comic: MangaItem,
}

impl Source for BikaSource {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut category = String::new();
		let mut sort = String::from("dd");

		for filter in filters {
			match filter {
				FilterValue::Select { id, value } => {
					if id == "category" {
						category = value;
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
			let resp: ComicsListResponse = helper::search(query, page)?;
			Ok(parser::build_result(
				&resp.data.comics.docs,
				&resp.data.comics,
			))
		} else {
			let url = helper::gen_explore_url(category, sort, page);
			let resp: ComicsListResponse = helper::get_json(url)?;
			Ok(parser::build_result(
				&resp.data.comics.docs,
				&resp.data.comics,
			))
		}
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			let url = helper::gen_manga_details_url(manga.key.clone());
			let resp: SingleMangaResponse = helper::get_json(url)?;
			let detailed = parser::manga_from_item(&resp.data.comic);
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
			let mut chapters: Vec<Chapter> = Vec::new();
			let mut page = 1;
			loop {
				let url = helper::gen_chapter_list_url(manga.key.clone(), page);
				let resp: PicaResponse<EpsResponse> = helper::get_json(url)?;
				let mut batch = parser::parse_chapter_list(&manga.key, &resp.data.eps.docs);
				chapters.append(&mut batch);
				if resp.data.eps.pages <= page {
					break;
				}
				page += 1;
			}
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let mut pages_out: Vec<Page> = Vec::new();
		let mut page = 1;
		loop {
			let url = helper::gen_page_list_url(manga.key.clone(), chapter.key.clone(), page);
			let resp: PicaResponse<PagesResponse> = helper::get_json(url)?;
			let offset = (page - 1) * resp.data.pages.limit;
			let mut batch = parser::parse_page_list(&resp.data.pages.docs, offset);
			pages_out.append(&mut batch);
			if resp.data.pages.pages <= page {
				break;
			}
			page += 1;
		}
		Ok(pages_out)
	}
}

impl ListingProvider for BikaSource {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let sort = String::from("dd");
		let (rank_time, is_random, category) = match listing.id.as_str() {
			"日榜" => (Some("H24"), false, String::new()),
			"周榜" => (Some("D7"), false, String::new()),
			"月榜" => (Some("D30"), false, String::new()),
			"随机本子" => (None, true, String::new()),
			"大湿推荐" => (None, false, String::from("大濕推薦")),
			"那年今天" => (None, false, String::from("那年今天")),
			"大家都在看" => (None, false, String::from("大家都在看")),
			"官方都在看" => (None, false, String::from("官方都在看")),
			_ => return self.get_search_manga_list(None, page, Vec::new()),
		};

		if let Some(time) = rank_time {
			let url = helper::gen_rank_url(time.into());
			let resp: RankComicsResponse = helper::get_json(url)?;
			Ok(MangaPageResult {
				entries: parser::parse_manga_list(&resp.data.comics),
				has_next_page: false,
			})
		} else if is_random {
			let url = helper::gen_random_url();
			let resp: RankComicsResponse = helper::get_json(url)?;
			Ok(MangaPageResult {
				entries: parser::parse_manga_list(&resp.data.comics),
				has_next_page: false,
			})
		} else {
			let url = helper::gen_explore_url(category, sort, page);
			let resp: ComicsListResponse = helper::get_json(url)?;
			Ok(parser::build_result(
				&resp.data.comics.docs,
				&resp.data.comics,
			))
		}
	}
}

register_source!(BikaSource, ListingProvider);
