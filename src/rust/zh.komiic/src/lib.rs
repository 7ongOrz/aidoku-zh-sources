#![no_std]
use aidoku::{
	alloc::{String, Vec},
	imports::{
		defaults::defaults_get,
		net::Request,
	},
	prelude::*,
	Chapter, FilterValue, ImageRequestProvider, Listing, ListingProvider, Manga, MangaPageResult,
	Page, Result, Source,
};
use aidoku::alloc::string::ToString;

mod helper;
mod parser;

use parser::{
	ChaptersByComicIdData, ComicByCategoriesData, ComicByIdData, GqlResponse, HotComicsData,
	ImagesByChapterIdData, RecentUpdateData, SearchData,
};

const FILTER_CATEGORY: [&str; 38] = [
	"", "1", "3", "4", "5", "6", "7", "8", "10", "11", "2", "12", "13", "14", "15", "16", "17",
	"18", "19", "20", "21", "22", "23", "24", "25", "26", "27", "9", "28", "31", "32", "33", "34",
	"35", "36", "37", "40", "42",
];
const FILTER_STATUS: [&str; 3] = ["", "ONGOING", "END"];
const FILTER_ORDER_BY: [&str; 3] = ["DATE_UPDATED", "VIEWS", "FAVORITE_COUNT"];

struct KomiicSource;

impl Source for KomiicSource {
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
		let mut status = String::new();
		let mut order_by = String::new();

		for filter in filters {
			match filter {
				FilterValue::Select { id, value } => {
					let index = value.parse::<usize>().unwrap_or(0);
					match id.as_str() {
						"category" => {
							category = FILTER_CATEGORY.get(index).unwrap_or(&"").to_string();
						}
						"status" => {
							status = FILTER_STATUS.get(index).unwrap_or(&"").to_string();
						}
						_ => {}
					}
				}
				FilterValue::Sort { id, index, .. } => {
					if id == "order" {
						order_by = FILTER_ORDER_BY
							.get(index as usize)
							.unwrap_or(&"DATE_UPDATED")
							.to_string();
					}
				}
				_ => {}
			}
		}

		if let Some(query) = query {
			let body = helper::gen_search_body_string(&query);
			let resp: GqlResponse<SearchData> = helper::get_json(&body)?;
			let entries = parser::parse_manga_list(&resp.data.search_comics_and_authors.comics);
			Ok(MangaPageResult {
				has_next_page: false,
				entries,
			})
		} else {
			let body = helper::gen_category_body_string(&category, &status, &order_by, page);
			let resp: GqlResponse<ComicByCategoriesData> = helper::get_json(&body)?;
			let entries = parser::parse_manga_list(&resp.data.comic_by_categories);
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
		if needs_details {
			let body = helper::gen_id_body_string(&manga.key);
			let resp: GqlResponse<ComicByIdData> = helper::get_json(&body)?;
			let detailed = parser::parse_manga(&resp.data.comic_by_id);
			manga.title = detailed.title;
			manga.cover = detailed.cover;
			manga.authors = detailed.authors;
			manga.url = detailed.url;
			manga.tags = detailed.tags;
			manga.status = detailed.status;
			manga.content_rating = detailed.content_rating;
			manga.viewer = detailed.viewer;
		}

		if needs_chapters {
			let body = helper::gen_chapter_body_string(&manga.key);
			let resp: GqlResponse<ChaptersByComicIdData> = helper::get_json(&body)?;
			manga.chapters =
				Some(parser::parse_chapter_list(&manga.key, &resp.data.chapters_by_comic_id));
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let body = helper::gen_images_body_string(&chapter.key);
		let resp: GqlResponse<ImagesByChapterIdData> = helper::get_json(&body)?;
		Ok(parser::parse_page_list(
			&manga.key,
			&chapter.key,
			&resp.data.images_by_chapter_id,
		))
	}
}

impl ListingProvider for KomiicSource {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		match listing.id.as_str() {
			"最近更新" => {
				let body = helper::gen_recent_update_body_string(page);
				let resp: GqlResponse<RecentUpdateData> = helper::get_json(&body)?;
				let entries = parser::parse_manga_list(&resp.data.recent_update);
				Ok(MangaPageResult {
					has_next_page: !entries.is_empty(),
					entries,
				})
			}
			"本月热门" => {
				let body = helper::gen_hot_body_string("MONTH_VIEWS", page);
				let resp: GqlResponse<HotComicsData> = helper::get_json(&body)?;
				let entries = parser::parse_manga_list(&resp.data.hot_comics);
				Ok(MangaPageResult {
					has_next_page: !entries.is_empty(),
					entries,
				})
			}
			"历史热门" => {
				let body = helper::gen_hot_body_string("VIEWS", page);
				let resp: GqlResponse<HotComicsData> = helper::get_json(&body)?;
				let entries = parser::parse_manga_list(&resp.data.hot_comics);
				Ok(MangaPageResult {
					has_next_page: !entries.is_empty(),
					entries,
				})
			}
			_ => self.get_search_manga_list(None, page, Vec::new()),
		}
	}
}

impl ImageRequestProvider for KomiicSource {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		let referer = helper::gen_referer(&url);
		let cookie = defaults_get::<String>("cookie").unwrap_or_default();
		let req = Request::get(&url)?.header("Referer", &referer);
		if cookie.is_empty() {
			Ok(req)
		} else {
			Ok(req.header("Cookie", &cookie))
		}
	}
}

register_source!(KomiicSource, ListingProvider, ImageRequestProvider);
