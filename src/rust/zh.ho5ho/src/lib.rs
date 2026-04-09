#![no_std]
use aidoku::{
	alloc::{String, Vec},
	helpers::uri::encode_uri,
	imports::net::Request,
	prelude::*,
	Chapter, ContentRating, FilterValue, ImageRequestProvider, Manga, MangaPageResult, MangaStatus,
	Page, PageContent, Result, Source, Viewer,
};
use aidoku::alloc::string::ToString;

const WWW_URL: &str = "https://www.ho5ho.com";
const MANGA_URL: &str = "https://www.ho5ho.com/%E4%B8%AD%E5%AD%97h%E6%BC%AB";

const FILTER_SORT: [&str; 3] = ["latest", "rating", "views"];

struct Ho5hoSource;

impl Source for Ho5hoSource {
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
		let mut sort = String::from("latest");

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

		let url = if let Some(query) = query {
			format!(
				"{}/page/{}/?s={}&post_type=wp-manga",
				WWW_URL,
				page,
				encode_uri(query)
			)
		} else if category.is_empty() {
			format!("{}/page/{}/?m_orderby={}", WWW_URL, page, sort)
		} else {
			format!(
				"{}/manga-genre/{}/page/{}/?m_orderby={}",
				WWW_URL,
				encode_uri(category),
				page,
				sort
			)
		};

		let html = Request::get(&url)?.html()?;
		let mut entries: Vec<Manga> = Vec::new();

		if let Some(items) = html.select("div[class*='c-image-hover']>a") {
			for item in items {
				let href = item.attr("href").unwrap_or_default();
				let parts: Vec<String> = href
					.split("/")
					.filter(|a| !a.is_empty())
					.map(|a| a.to_string())
					.collect();
				let key = parts.get(3).cloned().unwrap_or_default();
				let cover = item
					.select_first("img")
					.and_then(|e| e.attr("data-src"))
					.map(|s| encode_uri(s))
					.unwrap_or_default();
				let title = item.attr("title").unwrap_or_default();
				entries.push(Manga {
					key,
					cover: Some(cover),
					title,
					..Default::default()
				});
			}
		}

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
			let url = format!("{}/{}/", MANGA_URL, manga.key);
			let html = Request::get(&url)?.html()?;
			let cover = html
				.select_first(".summary_image>a>img")
				.and_then(|e| e.attr("data-src"))
				.unwrap_or_default()
				.replace("193x278", "175x238");
			manga.cover = Some(encode_uri(cover));
			manga.title = html
				.select_first(".post-title>h1")
				.and_then(|e| e.text())
				.unwrap_or_default();
			let mut authors: Vec<String> = Vec::new();
			if let Some(list) = html.select(".author-content>a") {
				for a in list {
					if let Some(name) = a.text() {
						authors.push(name);
					}
				}
			}
			manga.authors = Some(authors);
			let mut desc_parts: Vec<String> = Vec::new();
			if let Some(list) = html.select(".description-summary>div>p") {
				for a in list {
					if let Some(t) = a.text() {
						desc_parts.push(t);
					}
				}
			}
			manga.description = Some(desc_parts.join("\n"));
			let mut tags: Vec<String> = Vec::new();
			if let Some(list) = html.select(".genres-content>a") {
				for a in list {
					if let Some(t) = a.text() {
						tags.push(t);
					}
				}
			}
			manga.tags = Some(tags);
			manga.status = match html
				.select_first(".post-status>div:nth-child(2)>.summary-content")
				.and_then(|e| e.text())
				.unwrap_or_default()
				.trim()
			{
				"OnGoing" => MangaStatus::Ongoing,
				"Completed" => MangaStatus::Completed,
				_ => MangaStatus::Unknown,
			};
			manga.content_rating = ContentRating::NSFW;
			manga.viewer = Viewer::RightToLeft;
			manga.url = Some(url);
		}

		if needs_chapters {
			let url = format!("{}/{}/", MANGA_URL, manga.key);
			let html = Request::get(&url)?.html()?;
			let mut chapters: Vec<Chapter> = Vec::new();

			if let Some(list) = html.select(".wp-manga-chapter>a") {
				for (index, item) in list.enumerate() {
					let chapter_url = item.attr("href").unwrap_or_default();
					let key = chapter_url
						.split("/")
						.filter(|a| !a.is_empty())
						.map(|a| a.to_string())
						.collect::<Vec<String>>()
						.pop()
						.unwrap_or_default();
					let title = item.text().unwrap_or_default();
					chapters.push(Chapter {
						key,
						title: Some(title),
						chapter_number: Some((index + 1) as f32),
						url: Some(chapter_url),
						..Default::default()
					});
				}
			}
			chapters.reverse();
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{}/{}/{}/", MANGA_URL, manga.key, chapter.key);
		let text = Request::get(&url)?.string()?;
		let json_str = text
			.split_once("var chapter_preloaded_images = ")
			.map(|(_, after)| after)
			.and_then(|s| s.split_once(", chapter_images_per_page ="))
			.map(|(before, _)| before)
			.unwrap_or("");
		let urls: Vec<String> = serde_json::from_str(json_str).unwrap_or_default();
		let pages: Vec<Page> = urls
			.into_iter()
			.map(|url| Page {
				content: PageContent::url(url),
				..Default::default()
			})
			.collect();
		Ok(pages)
	}
}

impl ImageRequestProvider for Ho5hoSource {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		Ok(Request::get(&url)?.header("Referer", WWW_URL))
	}
}

register_source!(Ho5hoSource, ImageRequestProvider);
