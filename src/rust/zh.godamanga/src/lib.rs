#![no_std]
use aidoku::{
	alloc::{String, Vec},
	helpers::uri::encode_uri,
	imports::net::Request,
	prelude::*,
	serde::Deserialize,
	Chapter, ContentRating, FilterValue, ImageRequestProvider, Listing, ListingProvider, Manga,
	MangaPageResult, MangaStatus, Page, PageContent, Result, Source, Viewer,
};
use aidoku::alloc::string::ToString;

const WWW_URL: &str = "https://godamh.com";
const API_URL: &str = "https://api-get-v2.mgsearcher.com";
const IMG_URL: &str = "https://f40-1-4.g-mh.online";
const TITLE_SUFFIX: &str = "-G站漫畫";

fn handle_cover_url(url: String) -> String {
	if url.contains("url=") {
		url.split("url=")
			.map(|a| a.to_string())
			.collect::<Vec<String>>()
			.pop()
			.unwrap_or_default()
			.replace("%3A", ":")
			.replace("%2F", "/")
			.replace("&w=250&q=60", "")
	} else {
		url
	}
}

#[derive(Deserialize)]
struct ApiResponse<T> {
	data: T,
}

#[derive(Deserialize)]
struct MangaChapters {
	chapters: Vec<ApiChapter>,
}

#[derive(Deserialize)]
struct ApiChapter {
	id: i64,
	attributes: ApiChapterAttributes,
}

#[derive(Deserialize)]
struct ApiChapterAttributes {
	title: String,
	slug: String,
}

#[derive(Deserialize)]
struct ChapterInfoOuter {
	info: ChapterInfo,
}

#[derive(Deserialize)]
struct ChapterInfo {
	images: ChapterImages,
}

#[derive(Deserialize)]
struct ChapterImages {
	images: Vec<ChapterImage>,
}

#[derive(Deserialize)]
struct ChapterImage {
	url: String,
}

struct GodamangaSource;

fn parse_manga_list(html: aidoku::imports::html::Document) -> Result<MangaPageResult> {
	let mut entries: Vec<Manga> = Vec::new();
	if let Some(items) = html.select(".pb-2>a") {
		for item in items {
			let key = item
				.attr("href")
				.unwrap_or_default()
				.split('/')
				.map(|a| a.to_string())
				.collect::<Vec<String>>()
				.pop()
				.unwrap_or_default();
			let cover = item
				.select_first("div>img")
				.and_then(|e| e.attr("src"))
				.map(handle_cover_url);
			let title = item.select_first("div>h3").and_then(|e| e.text()).unwrap_or_default();
			entries.push(Manga {
				key,
				cover,
				title,
				..Default::default()
			});
		}
	}
	let has_next_page = !entries.is_empty();
	Ok(MangaPageResult { entries, has_next_page })
}

impl Source for GodamangaSource {
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
		for filter in filters {
			if let FilterValue::Select { id, value } = filter {
				if id == "category" {
					category = value;
				}
			}
		}

		let url = if let Some(query) = query {
			format!("{}/s/{}?page={}", WWW_URL, encode_uri(query), page)
		} else {
			let category_str = if category.is_empty() {
				String::from("manga")
			} else if category.len() <= 2 {
				format!("manga-genre/{}", category)
			} else {
				format!("manga-tag/{}", category)
			};
			format!("{}/{}/page/{}", WWW_URL, category_str, page)
		};
		let html = Request::get(&url)?.html()?;
		parse_manga_list(html)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let ids: Vec<String> = manga.key.split('/').map(|s| s.to_string()).collect();
		let manga_id = ids.first().cloned().unwrap_or_default();
		let mut mid = ids.get(1).cloned().unwrap_or_default();
		let should_fetch_manga_page = needs_details || (needs_chapters && mid.is_empty());
		let mut details_html = None;

		if should_fetch_manga_page {
			let url = format!("{}/manga/{}", WWW_URL, manga_id);
			let html = Request::get(&url)?.html()?;
			let fetched_mid = html
				.select_first("#mangachapters")
				.and_then(|e| e.attr("data-mid"))
				.unwrap_or_default();
			if mid.is_empty() {
				mid = fetched_mid.clone();
			}
			details_html = Some((url, html));
		}

		if needs_details {
			let (url, html) = details_html.take().unwrap();
			manga.cover = html
				.select_first("meta[property='og:image']")
				.and_then(|e| e.attr("content"))
				.map(handle_cover_url);
			manga.title = html
				.select_first("title")
				.and_then(|e| e.text())
				.unwrap_or_default()
				.replace(TITLE_SUFFIX, "");
			let authors = html
				.select("a[href*=author]>span")
				.map(|items| {
					items
						.filter_map(|a| a.text().map(|t| t.replace(",", "")))
						.filter(|a| !a.is_empty())
						.collect::<Vec<String>>()
				})
				.unwrap_or_default();
			manga.authors = Some(authors);
			manga.description = html.select_first(".text-medium.my-unit-md").and_then(|e| e.text());
			let tags = html
				.select(".py-1>a:not([href*=author])>span")
				.map(|items| {
					items
						.filter_map(|a| a.text())
						.map(|t| t.replace(",", "").replace("热门漫画", "").replace("#", "").replace("热门推荐", "").trim().to_string())
						.filter(|a| !a.is_empty())
						.collect::<Vec<String>>()
				})
				.unwrap_or_default();
			manga.tags = Some(tags);
			manga.status = MangaStatus::Ongoing;
			manga.content_rating = ContentRating::Safe;
			manga.viewer = Viewer::Webtoon;
			manga.url = Some(url);
		}

		if !mid.is_empty() {
			manga.key = format!("{}/{}", manga_id, mid);
		}

		if needs_chapters {
			let url = format!("{}/api/manga/get?mid={}&mode=all", API_URL, mid);
			let data: ApiResponse<MangaChapters> = Request::get(&url)?
				.header("Origin", WWW_URL)
				.header("Referer", WWW_URL)
				.json_owned()?;
			let mut chapters: Vec<Chapter> = Vec::new();
			for (index, item) in data.data.chapters.into_iter().enumerate() {
				let chapter_url = format!("{}/manga/{}/{}", WWW_URL, manga_id, item.attributes.slug);
				chapters.push(Chapter {
					key: item.id.to_string(),
					title: Some(item.attributes.title),
					chapter_number: Some((index + 1) as f32),
					url: Some(chapter_url),
					..Default::default()
				});
			}
			chapters.reverse();
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let ids: Vec<String> = manga.key.split('/').map(|s| s.to_string()).collect();
		let mid = ids.get(1).cloned().unwrap_or_default();
		let url = format!("{}/api/chapter/getinfo?m={}&c={}", API_URL, mid, chapter.key);
		let data: ApiResponse<ChapterInfoOuter> = Request::get(&url)?
			.header("Origin", WWW_URL)
			.header("Referer", WWW_URL)
			.json_owned()?;
		let pages = data
			.data
			.info
			.images
			.images
			.into_iter()
			.map(|item| Page {
				content: PageContent::url(format!("{}{}", IMG_URL, item.url)),
				..Default::default()
			})
			.collect();
		Ok(pages)
	}
}

impl ListingProvider for GodamangaSource {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let list = match listing.id.as_str() {
			"人气推荐" => "hots",
			"热门更新" => "dayup",
			"最新上架" => "newss",
			_ => return self.get_search_manga_list(None, page, Vec::new()),
		};
		let url = format!("{}/{}/page/{}", WWW_URL, list, page);
		let html = Request::get(&url)?.html()?;
		parse_manga_list(html)
	}
}

impl ImageRequestProvider for GodamangaSource {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		Ok(Request::get(&url)?.header("Referer", WWW_URL))
	}
}

register_source!(GodamangaSource, ListingProvider, ImageRequestProvider);
