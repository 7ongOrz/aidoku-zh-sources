#![no_std]
use aidoku::{
	alloc::{String, Vec},
	imports::net::{HttpMethod, Request},
	prelude::*,
	Chapter, ContentRating, FilterValue, ImageRequestProvider, Listing, ListingProvider, Manga,
	MangaPageResult, MangaStatus, Page, PageContent, Result, Source, Viewer,
};
use aidoku::alloc::string::ToString;
use serde::Deserialize;

const WWW_URL: &str = "https://m.happymh.com";
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

const FILTER_ORDER: [&str; 2] = ["last_date", "views"];

#[derive(Deserialize)]
struct ListResponse {
	data: ListData,
}

#[derive(Deserialize)]
struct ListData {
	items: Vec<ListItem>,
}

#[derive(Deserialize)]
struct ListItem {
	manga_code: String,
	cover: String,
	name: String,
}

#[derive(Deserialize)]
struct ChapterResponse {
	data: ChapterPage,
}

#[derive(Deserialize)]
struct ChapterPage {
	#[serde(rename = "isEnd")]
	is_end: i64,
	items: Vec<ChapterItem>,
}

#[derive(Deserialize)]
struct ChapterItem {
	codes: String,
	#[serde(rename = "chapterName")]
	chapter_name: String,
}

#[derive(Deserialize)]
struct ReadingResponse {
	data: ReadingData,
}

#[derive(Deserialize)]
struct ReadingData {
	scans: Vec<ReadingScan>,
}

#[derive(Deserialize)]
struct ReadingScan {
	url: String,
}

fn req(url: &str, referer: &str) -> core::result::Result<Request, aidoku::imports::net::RequestError> {
	Ok(Request::get(url)?
		.header("User-Agent", UA)
		.header("Referer", referer)
		.header("Origin", WWW_URL))
}

fn fetch_all_chapters(key: &str) -> Result<Vec<ChapterItem>> {
	let mut all = Vec::new();
	let mut page = 1;
	loop {
		let url = format!(
			"{}/v2.0/apis/manga/chapterByPage?code={}&page={}&lang=cn&order=asc",
			WWW_URL, key, page
		);
		let referer = format!("{}/manga/{}", WWW_URL, key);
		let resp: ChapterResponse = req(&url, &referer)?.json_owned()?;
		let is_end = resp.data.is_end;
		all.extend(resp.data.items);
		if is_end == 1 {
			break;
		}
		page += 1;
	}
	Ok(all)
}

struct HappymhSource;

impl Source for HappymhSource {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut genre = String::new();
		let mut area = String::new();
		let mut audience = String::new();
		let mut status = String::from("-1");
		let mut order = String::from("last_date");

		for filter in filters {
			match filter {
				FilterValue::Select { id, value } => match id.as_str() {
					"genre" => genre = value,
					"area" => area = value,
					"audience" => audience = value,
					"status" => status = value,
					_ => {}
				},
				FilterValue::Sort { id, index, .. } => {
					if id == "sort" {
						if let Some(s) = FILTER_ORDER.get(index as usize) {
							order = s.to_string();
						}
					}
				}
				_ => {}
			}
		}

		let resp: ListResponse = if let Some(query) = query {
			let url = format!("{}/v2.0/apis/manga/ssearch", WWW_URL);
			let body = format!("searchkey={}&v=v2.13", query);
			let referer = format!("{}/sssearch", WWW_URL);
			Request::new(&url, HttpMethod::Post)?
				.header("User-Agent", UA)
				.header("Content-Type", "application/x-www-form-urlencoded")
				.header("Referer", &referer)
				.header("Origin", WWW_URL)
				.body(body.as_bytes())
				.json_owned()?
		} else {
			let url = format!(
				"{}/apis/c/index?genre={}&area={}&audience={}&series_status={}&order={}&pn={}",
				WWW_URL, genre, area, audience, status, order, page
			);
			let referer = format!("{}/latest", WWW_URL);
			req(&url, &referer)?.json_owned()?
		};

		let entries: Vec<Manga> = resp
			.data
			.items
			.into_iter()
			.map(|item| Manga {
				key: item.manga_code,
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

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			let url = format!("{}/manga/{}", WWW_URL, manga.key);
			let referer = format!("{}/latest", WWW_URL);
			let html = req(&url, &referer)?.html()?;

			manga.cover = html
				.select_first(".mg-cover>mip-img")
				.and_then(|e| e.attr("src"));
			manga.title = html
				.select_first("h2.mg-title")
				.and_then(|e| e.text())
				.unwrap_or_default();

			let mut authors = Vec::new();
			if let Some(items) = html.select(".mg-sub-title>a") {
				for item in items {
					if let Some(t) = item.text() {
						authors.push(t);
					}
				}
			}
			manga.authors = Some(authors);

			manga.description = html
				.select_first("#showmore")
				.and_then(|e| e.text())
				.map(|s| s.trim().to_string());

			let mut tags = Vec::new();
			if let Some(items) = html.select(".mg-cate>a") {
				for item in items {
					if let Some(t) = item.text() {
						tags.push(t);
					}
				}
			}
			manga.tags = Some(tags);

			manga.status = MangaStatus::Unknown;
			manga.content_rating = ContentRating::Safe;
			manga.viewer = Viewer::Webtoon;
			manga.url = Some(url);
		}

		if needs_chapters {
			let items = fetch_all_chapters(&manga.key)?;
			let len = items.len();
			let mut chapters: Vec<Chapter> = items
				.into_iter()
				.enumerate()
				.map(|(index, item)| Chapter {
					key: item.codes.clone(),
					title: Some(item.chapter_name),
					chapter_number: Some((index + 1) as f32),
					url: Some(format!("{}/mangaread/{}", WWW_URL, item.codes)),
					..Default::default()
				})
				.collect();
			if len > 1 {
				chapters.reverse();
			}
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!(
			"{}/v2.0/apis/manga/reading?code={}&v=v3.1818134",
			WWW_URL, chapter.key
		);
		let referer = format!("{}/mangaread/{}", WWW_URL, chapter.key);
		let resp: ReadingResponse = Request::get(&url)?
			.header("User-Agent", UA)
			.header("Referer", &referer)
			.header("Origin", WWW_URL)
			.header("X-Requested-With", "XMLHttpRequest")
			.json_owned()?;

		let pages: Vec<Page> = resp
			.data
			.scans
			.into_iter()
			.map(|scan| Page {
				content: PageContent::url(scan.url),
				..Default::default()
			})
			.collect();

		Ok(pages)
	}
}

impl ListingProvider for HappymhSource {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let slug = match listing.id.as_str() {
			"日阅读" => "day",
			"日收藏" => "dayBookcases",
			"周阅读" => "week",
			"周收藏" => "weekBookcase",
			"月阅读" => "month",
			"月收藏" => "monthBookcases",
			"总评分" => "voteRank",
			"月投票" => "voteNumMonthRank",
			_ => return self.get_search_manga_list(None, page, Vec::new()),
		};

		let url = format!("{}/rank/{}", WWW_URL, slug);
		let html = req(&url, &url)?.html()?;

		let mut entries: Vec<Manga> = Vec::new();
		if let Some(items) = html.select(".manga-rank") {
			for item in items {
				let key = item
					.select_first(".manga-rank-cover>a")
					.and_then(|e| e.attr("href"))
					.unwrap_or_default()
					.split("/")
					.filter(|a| !a.is_empty())
					.map(|a| a.to_string())
					.collect::<Vec<String>>()
					.pop()
					.unwrap_or_default();
				let cover = item
					.select_first(".manga-rank-cover>a>mip-img")
					.and_then(|e| e.attr("src"));
				let title = item
					.select_first(".manga-title")
					.and_then(|e| e.text())
					.unwrap_or_default()
					.trim()
					.to_string();
				entries.push(Manga {
					key,
					title,
					cover,
					..Default::default()
				});
			}
		}

		Ok(MangaPageResult {
			has_next_page: false,
			entries,
		})
	}
}

impl ImageRequestProvider for HappymhSource {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		Ok(Request::get(&url)?
			.header("Referer", WWW_URL)
			.header("User-Agent", UA)
			.header("Origin", WWW_URL))
	}
}

register_source!(HappymhSource, ListingProvider, ImageRequestProvider);
