#![no_std]
use aidoku::{
	alloc::{String, Vec},
	helpers::uri::encode_uri,
	imports::{
		defaults::defaults_get,
		net::Request,
	},
	prelude::*,
	Chapter, ContentRating, FilterValue, ImageRequestProvider, Listing, ListingProvider, Manga,
	MangaPageResult, MangaStatus, Page, PageContent, Result, Source, Viewer,
};
use aidoku::alloc::string::ToString;
use serde::Deserialize;

const WWW_URL: &str = "https://www.mkzhan.com";
const API_URL: &str = "https://comic.mkzcdn.com";

const FILTER_ORDER: [&str; 3] = ["3", "1", "2"];

#[derive(Deserialize)]
struct ApiResponse {
	data: ApiData,
}

#[derive(Deserialize)]
struct ApiData {
	list: Vec<ComicItem>,
}

#[derive(Deserialize)]
struct ComicItem {
	comic_id: String,
	cover: String,
	title: String,
}

#[derive(Deserialize)]
struct ChapterApiResponse {
	data: Vec<ChapterItem>,
}

#[derive(Deserialize)]
struct ChapterItem {
	chapter_id: String,
	title: String,
}

#[derive(Deserialize)]
struct PageApiResponse {
	data: PageData,
}

#[derive(Deserialize)]
struct PageData {
	page: Vec<PageItem>,
}

#[derive(Deserialize)]
struct PageItem {
	image: String,
}

fn parse_comic_list(items: Vec<ComicItem>) -> MangaPageResult {
	let entries: Vec<Manga> = items
		.into_iter()
		.map(|item| Manga {
			key: item.comic_id,
			cover: Some(item.cover),
			title: item.title,
			..Default::default()
		})
		.collect();
	MangaPageResult {
		has_next_page: !entries.is_empty(),
		entries,
	}
}

struct MkzhanSource;

impl Source for MkzhanSource {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut theme_id = String::from("0");
		let mut finish = String::new();
		let mut audience = String::new();
		let mut copyright = String::new();
		let mut free = String::new();
		let mut order = String::from("3");

		for filter in filters {
			match filter {
				FilterValue::Select { id, value } => match id.as_str() {
					"theme" => theme_id = value,
					"finish" => finish = value,
					"audience" => audience = value,
					"copyright" => copyright = value,
					"free" => free = value,
					_ => {}
				},
				FilterValue::Sort { id, index, .. } => {
					if id == "order" {
						if let Some(s) = FILTER_ORDER.get(index as usize) {
							order = s.to_string();
						}
					}
				}
				_ => {}
			}
		}

		let url = if let Some(query) = query {
			format!(
				"{}/search/keyword/?keyword={}&page_num={}&page_size=20",
				API_URL,
				encode_uri(query),
				page
			)
		} else {
			let mut base = format!(
				"{}/search/filter/?theme_id={}&order={}&page_num={}&page_size=15",
				API_URL, theme_id, order, page
			);
			if !finish.is_empty() {
				base.push_str(&format!("&finish={}", finish));
			}
			if !audience.is_empty() {
				base.push_str(&format!("&audience={}", audience));
			}
			if !copyright.is_empty() {
				base.push_str(&format!("&copyright={}", copyright));
			}
			if !free.is_empty() {
				base.push_str(&format!("&{}", free));
			}
			base
		};

		let resp: ApiResponse = Request::get(&url)?.json_owned()?;
		Ok(parse_comic_list(resp.data.list))
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			let url = format!("{}/{}/", WWW_URL, manga.key);
			let html = Request::get(&url)?.html()?;

			manga.cover = html
				.select_first(".de-info__cover>img")
				.and_then(|e| e.attr("data-src"))
				.map(|s| s.replace("!cover-400", ""));
			manga.title = html
				.select_first(".j-comic-title")
				.and_then(|e| e.text())
				.unwrap_or_default();

			let mut authors = Vec::new();
			if let Some(items) = html.select(".comic-author>.name>a") {
				for item in items {
					if let Some(t) = item.text() {
						authors.push(t);
					}
				}
			}
			manga.authors = Some(authors);

			manga.description = html
				.select_first(".intro-total")
				.and_then(|e| e.text())
				.map(|s| s.trim().to_string());

			let tags_text = html
				.select_first(".comic-status>span:nth-child(1)>b")
				.and_then(|e| e.text())
				.unwrap_or_default();
			let tags: Vec<String> = tags_text
				.split(" ")
				.map(|a| a.to_string())
				.collect();
			manga.tags = Some(tags);

			manga.status = match html
				.select_first(".de-chapter__title>span:nth-child(1)")
				.and_then(|e| e.text())
				.unwrap_or_default()
				.as_str()
			{
				"连载" => MangaStatus::Ongoing,
				"完结" => MangaStatus::Completed,
				_ => MangaStatus::Unknown,
			};
			manga.content_rating = ContentRating::Safe;
			manga.viewer = Viewer::Webtoon;
			manga.url = Some(url);
		}

		if needs_chapters {
			let url = format!("{}/chapter/v1/?comic_id={}", API_URL, manga.key);
			let resp: ChapterApiResponse = Request::get(&url)?.json_owned()?;
			let mut chapters: Vec<Chapter> = Vec::new();

			for (index, item) in resp.data.iter().enumerate() {
				let chapter_url = format!(
					"{}/{}/{}.html",
					WWW_URL, manga.key, item.chapter_id
				);
				chapters.push(Chapter {
					key: item.chapter_id.clone(),
					title: Some(item.title.clone()),
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
		let uid: String = defaults_get("uid").unwrap_or_default();
		let sign: String = defaults_get("sign").unwrap_or_default();
		let url = format!(
			"{}/chapter/content/v1/?comic_id={}&chapter_id={}&format=1&quality=1&type=1&uid={}&sign={}",
			API_URL, manga.key, chapter.key, uid, sign
		);
		let resp: PageApiResponse = Request::get(&url)?.json_owned()?;
		let pages: Vec<Page> = resp
			.data
			.page
			.into_iter()
			.map(|item| Page {
				content: PageContent::url(item.image),
				..Default::default()
			})
			.collect();

		Ok(pages)
	}
}

impl ListingProvider for MkzhanSource {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let name = match listing.id.as_str() {
			"人气榜" => "popular",
			"收藏榜" => "collection",
			"新作榜" => "latest",
			"上升榜" => "ascension",
			"月票榜" => "ticket",
			"打赏榜" => "gratuity",
			"评分榜" => "score",
			"付费榜" => "popular/pay",
			_ => return self.get_search_manga_list(None, page, Vec::new()),
		};

		let url = format!(
			"{}/top/{}/?type=1&page_num={}&page_size=10",
			API_URL, name, page
		);
		let resp: ApiResponse = Request::get(&url)?.json_owned()?;
		Ok(parse_comic_list(resp.data.list))
	}
}

impl ImageRequestProvider for MkzhanSource {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		Ok(Request::get(&url)?.header("Referer", WWW_URL))
	}
}

register_source!(MkzhanSource, ListingProvider, ImageRequestProvider);
