#![no_std]
use aidoku::{
	alloc::{String, Vec},
	helpers::uri::encode_uri,
	imports::net::Request,
	prelude::*,
	Chapter, ContentRating, FilterValue, Listing, ListingProvider, Manga, MangaPageResult,
	MangaStatus, Page, PageContent, Result, Source, Viewer,
};
use serde::Deserialize;

const WWW_URL: &str = "https://m.zaimanhua.com";
const API_URL: &str = "https://manhua.zaimanhua.com/api/v1";
const APP_URL: &str = "https://manhua.zaimanhua.com/app/v1";
const V4_APP_URL: &str = "https://v4api.zaimanhua.com/app/v1";

// --- Filter list response ---

#[derive(Deserialize)]
struct FilterResponse {
	data: FilterData,
}

#[derive(Deserialize)]
struct FilterData {
	#[serde(rename = "comicList", default)]
	comic_list: Vec<ComicItem>,
}

#[derive(Deserialize)]
struct ComicItem {
	id: String,
	cover: String,
	name: String,
}

// --- Search response ---

#[derive(Deserialize)]
struct SearchResponse {
	data: SearchData,
}

#[derive(Deserialize)]
struct SearchData {
	list: Vec<SearchItem>,
}

#[derive(Deserialize)]
struct SearchItem {
	id: String,
	cover: String,
	title: String,
}

// --- Rank response ---

#[derive(Deserialize)]
struct RankResponse {
	data: RankData,
}

#[derive(Deserialize)]
struct RankData {
	list: Vec<RankItem>,
}

#[derive(Deserialize)]
struct RankItem {
	comic_id: String,
	cover: String,
	title: String,
}

// --- Detail response ---

#[derive(Deserialize)]
struct DetailResponse {
	data: DetailOuter,
}

#[derive(Deserialize)]
struct DetailOuter {
	data: DetailData,
}

#[derive(Deserialize)]
struct DetailData {
	cover: String,
	title: String,
	authors: Vec<TagItem>,
	description: String,
	types: Vec<TagItem>,
	status: Vec<TagItem>,
	chapters: Vec<ChapterGroup>,
}

#[derive(Deserialize)]
struct TagItem {
	tag_name: String,
}

#[derive(Deserialize)]
struct ChapterGroup {
	data: Vec<ChapterItem>,
}

#[derive(Deserialize)]
struct ChapterItem {
	chapter_id: String,
	chapter_title: String,
}

// --- Page list response ---

#[derive(Deserialize)]
struct PageResponse {
	data: PageOuter,
}

#[derive(Deserialize)]
struct PageOuter {
	#[serde(rename = "chapterInfo")]
	chapter_info: ChapterInfo,
}

#[derive(Deserialize)]
struct ChapterInfo {
	page_url: Vec<String>,
}

struct ZaimanhuaSource;

impl Source for ZaimanhuaSource {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut status = String::from("0");
		let mut audience = String::from("0");
		let mut theme = String::from("0");
		let mut cate = String::from("0");
		let mut first_letter = String::new();
		let mut sort_type = String::from("0");

		for filter in filters {
			match filter {
				FilterValue::Select { id, value } => match id.as_str() {
					"status" => status = value,
					"audience" => audience = value,
					"theme" => theme = value,
					"cate" => cate = value,
					"firstLetter" => first_letter = value,
					_ => {}
				},
				FilterValue::Sort { id, index, .. } => {
					if id == "sort" {
						if index == 0 {
							sort_type = String::from("0");
						}
					}
				}
				_ => {}
			}
		}

		if let Some(query) = query {
			let url = format!(
				"{}/search/index?keyword={}&source=0&page={}&size=20",
				APP_URL,
				encode_uri(query),
				page
			);
			let resp: SearchResponse = Request::get(&url)?.json_owned()?;
			let entries: Vec<Manga> = resp
				.data
				.list
				.into_iter()
				.map(|item| Manga {
					key: item.id,
					title: item.title,
					cover: Some(item.cover),
					..Default::default()
				})
				.collect();
			Ok(MangaPageResult {
				has_next_page: !entries.is_empty(),
				entries,
			})
		} else {
			let url = format!(
				"{}/comic1/filter?sortType={}&page={}&size=18&status={}&audience={}&theme={}&cate={}&firstLetter={}",
				API_URL, sort_type, page, status, audience, theme, cate, first_letter
			);
			let resp: FilterResponse = Request::get(&url)?.json_owned()?;
			let entries: Vec<Manga> = resp
				.data
				.comic_list
				.into_iter()
				.map(|item| Manga {
					key: item.id,
					title: item.name,
					cover: Some(item.cover),
					..Default::default()
				})
				.collect();
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
		if needs_details || needs_chapters {
			let url = format!("{}/comic/detail/{}", V4_APP_URL, manga.key);
			let resp: DetailResponse = Request::get(&url)?.json_owned()?;
			let detail = resp.data.data;

			if needs_details {
				manga.cover = Some(detail.cover);
				manga.title = detail.title;
				manga.authors = Some(
					detail
						.authors
						.iter()
						.map(|a| a.tag_name.clone())
						.collect(),
				);
				manga.description = Some(detail.description);
				manga.url = Some(format!("{}/pages/comic/detail?id={}", WWW_URL, manga.key));
				manga.tags = Some(
					detail
						.types
						.iter()
						.map(|a| a.tag_name.clone())
						.collect(),
				);
				manga.status = detail
					.status
					.first()
					.map(|s| match s.tag_name.as_str() {
						"连载中" => MangaStatus::Ongoing,
						"已完结" => MangaStatus::Completed,
						_ => MangaStatus::Unknown,
					})
					.unwrap_or(MangaStatus::Unknown);
				manga.content_rating = ContentRating::Safe;
				manga.viewer = Viewer::RightToLeft;
			}

			if needs_chapters {
				if let Some(group) = detail.chapters.first() {
					let len = group.data.len();
					let mut chapters: Vec<Chapter> = group
						.data
						.iter()
						.enumerate()
						.map(|(index, item)| Chapter {
							key: item.chapter_id.clone(),
							title: Some(item.chapter_title.clone()),
							chapter_number: Some((len - index) as f32),
							url: Some(format!(
								"{}/pages/comic/page?comic_id={}&chapter_id={}",
								WWW_URL, manga.key, item.chapter_id
							)),
							..Default::default()
						})
						.collect();
					chapters.reverse();
					manga.chapters = Some(chapters);
				}
			}
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!(
			"{}/comic1/chapter/detail?channel=pc&app_name=zmh&version=1.0.0&comic_id={}&chapter_id={}",
			API_URL, manga.key, chapter.key
		);
		let resp: PageResponse = Request::get(&url)?.json_owned()?;
		let pages: Vec<Page> = resp
			.data
			.chapter_info
			.page_url
			.into_iter()
			.map(|url| Page {
				content: PageContent::url(url),
				..Default::default()
			})
			.collect();
		Ok(pages)
	}
}

impl ListingProvider for ZaimanhuaSource {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let (cate, duration) = match listing.id.as_str() {
			"周人气排行" => ("1", "1"),
			"月人气排行" => ("1", "2"),
			"总人气排行" => ("1", "3"),
			"周点击排行" => ("2", "1"),
			"月点击排行" => ("2", "2"),
			"总点击排行" => ("2", "3"),
			_ => return self.get_search_manga_list(None, page, Vec::new()),
		};

		let url = format!(
			"{}/comic1/rank_list?channel=pc&app_name=zmh&version=1.0.0&page={}&size=10&duration={}&cate={}&tag=0&theme=0",
			API_URL, page, duration, cate
		);
		let resp: RankResponse = Request::get(&url)?.json_owned()?;
		let entries: Vec<Manga> = resp
			.data
			.list
			.into_iter()
			.map(|item| Manga {
				key: item.comic_id,
				title: item.title,
				cover: Some(item.cover),
				..Default::default()
			})
			.collect();
		Ok(MangaPageResult {
			has_next_page: !entries.is_empty(),
			entries,
		})
	}
}

register_source!(ZaimanhuaSource, ListingProvider);
