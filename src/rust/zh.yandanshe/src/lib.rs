#![no_std]
use aidoku::{
	alloc::{String, Vec},
	helpers::uri::encode_uri,
	imports::net::Request,
	prelude::*,
	Chapter, ContentRating, FilterValue, ImageRequestProvider, Manga, MangaPageResult,
	MangaStatus, Page, PageContent, Result, Source, Viewer,
};
use aidoku::alloc::string::ToString;

const WWW_URL: &str = "https://yandanshe.com";
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

struct YandansheSource;

fn parse_manga_list(html: aidoku::imports::html::Document) -> Result<MangaPageResult> {
	let mut entries: Vec<Manga> = Vec::new();
	if let Some(items) = html.select("article") {
		for item in items {
			let key = item
				.select_first("h3>a")
				.and_then(|e| e.attr("href"))
				.unwrap_or_default()
				.split('/')
				.map(|a| a.to_string())
				.filter(|a| !a.is_empty())
				.collect::<Vec<String>>()
				.pop()
				.unwrap_or_default();
			let cover = item.select_first(".thumbnail>a>img").and_then(|e| e.attr("src"));
			let title = item.select_first("h3>a").and_then(|e| e.text()).unwrap_or_default();
			entries.push(Manga {
				key,
				cover,
				title,
				..Default::default()
			});
		}
	}
	let has_next_page = !entries.is_empty();
	Ok(MangaPageResult {
		entries,
		has_next_page,
	})
}

impl Source for YandansheSource {
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
		let mut tag = String::new();
		let mut sort = String::from("time");

		for filter in filters {
			match filter {
				FilterValue::Select { id, value } => match id.as_str() {
					"category" => category = value,
					"status" => status = value,
					"tag" => tag = value,
					_ => {}
				},
				FilterValue::Sort { id, index, .. } => {
					if id == "sort" {
						sort = if index == 1 { String::from("like") } else { String::from("time") };
					}
				}
				_ => {}
			}
		}

		if category.is_empty() || status.is_empty() {
			category = String::new();
			status = String::new();
		}

		let url = if let Some(query) = query {
			format!("{}/page/{}/?s={}", WWW_URL, page, encode_uri(query))
		} else {
			format!(
				"{}/{}{}/page/{}/?tag={}&sort={}",
				WWW_URL,
				category,
				status,
				page,
				encode_uri(tag),
				sort
			)
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
		let base_url = format!("{}/{}", WWW_URL, manga.key);
		let html = if needs_details || needs_chapters {
			Some(Request::get(&base_url)?.html()?)
		} else {
			None
		};

		if needs_details {
			let html = html.as_ref().unwrap();
			manga.title = html
				.select_first("h1.article-title")
				.and_then(|e| e.text())
				.unwrap_or_default();
			manga.authors = Some(
				html.select(".article-meta>.item-author")
					.map(|items| items.filter_map(|a| a.text()).collect::<Vec<String>>())
					.unwrap_or_default(),
			);
			manga.description = html
				.select_first(".article-content>blockquote>p:nth-child(2)")
				.and_then(|e| e.text())
				.map(|s| s.replace("內容簡介：", ""));
			let mut categories = html
				.select(".article-meta>.item-cat>a")
				.map(|items| {
					items
						.filter_map(|a| a.text())
						.flat_map(|s| s.trim().split('·').map(|x| x.to_string()).collect::<Vec<String>>())
						.collect::<Vec<String>>()
				})
				.unwrap_or_default();
			let mut tags = html
				.select(".article-tags>.inner>a")
				.map(|items| items.filter_map(|a| a.text()).collect::<Vec<String>>())
				.unwrap_or_default();
			let status = categories.pop().unwrap_or_default();
			manga.status = match status.as_str() {
				"連載" => MangaStatus::Ongoing,
				"完結" => MangaStatus::Completed,
				_ => MangaStatus::Unknown,
			};
			manga.content_rating = ContentRating::NSFW;
			manga.viewer = Viewer::RightToLeft;
			categories.append(&mut tags);
			manga.tags = Some(categories);
			manga.url = Some(base_url.clone());
		}

		if needs_chapters {
			let html = html.as_ref().unwrap();
			let mut chapter_keys: Vec<String> = Vec::new();
			if let Some(items) = html.select(".list>*") {
				for item in items {
					chapter_keys.push(item.text().unwrap_or_default().trim().to_string());
				}
			}
			let mut chapters: Vec<Chapter> = Vec::new();
			if chapter_keys.is_empty() {
				chapters.push(Chapter {
					key: String::from("1"),
					title: Some(String::from("第 1 话")),
					chapter_number: Some(1.0),
					url: Some(format!("{}/{}/", base_url, 1)),
					..Default::default()
				});
			} else {
				for (index, key) in chapter_keys.into_iter().enumerate() {
					chapters.push(Chapter {
						key: key.clone(),
						title: Some(format!("第 {} 话", key)),
						chapter_number: Some((index + 1) as f32),
						url: Some(format!("{}/{}/", base_url, key)),
						..Default::default()
					});
				}
				chapters.reverse();
			}
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{}/{}/{}/", WWW_URL, manga.key, chapter.key);
		let html = Request::get(&url)?.html()?;
		let mut pages: Vec<Page> = Vec::new();
		if let Some(items) = html.select(".article-content>p>img") {
			for item in items {
				let url = item.attr("data-src").unwrap_or_default().trim().to_string();
				pages.push(Page {
					content: PageContent::url(url),
					..Default::default()
				});
			}
		}
		Ok(pages)
	}
}

impl ImageRequestProvider for YandansheSource {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		Ok(Request::get(&url)?.header("Referer", WWW_URL).header("User-Agent", UA))
	}
}

register_source!(YandansheSource, ImageRequestProvider);
