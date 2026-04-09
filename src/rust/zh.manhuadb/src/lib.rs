#![no_std]
use aidoku::{
	alloc::{String, Vec},
	helpers::uri::encode_uri,
	imports::net::Request,
	prelude::*,
	Chapter, ContentRating, FilterValue, Manga, MangaPageResult, MangaStatus, Page, PageContent,
	Result, Source, Viewer,
};
use aidoku::alloc::string::ToString;
use base64::{engine::general_purpose, Engine};
use serde::Deserialize;

const WWW_URL: &str = "https://www.manhuadb.com";
const STATIC_URL: &str = "https://i2.manhuadb.com/static";

fn handle_cover(mut cover: String) -> String {
	if !cover.starts_with("https") {
		cover = format!("{}{}", WWW_URL, cover)
	}
	cover
}

#[derive(Deserialize)]
struct PageImg {
	img: String,
}

struct ManhuadbSource;

impl Source for ManhuadbSource {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut region = String::new();
		let mut audience = String::new();
		let mut status = String::new();
		let mut category = String::new();

		for filter in filters {
			if let FilterValue::Select { id, value } = filter {
				match id.as_str() {
					"region" => region = value,
					"audience" => audience = value,
					"status" => status = value,
					"category" => category = value,
					_ => {}
				}
			}
		}

		let is_search = query.is_some();
		let url = if let Some(query) = query {
			format!("{}/search?q={}&p={}", WWW_URL, encode_uri(query), page)
		} else {
			format!(
				"{}/manhua/list-r-{}-a-{}-s-{}-c-{}-page-{}.html",
				WWW_URL, region, audience, status, category, page
			)
		};
		let class = if is_search { "comicbook" } else { "comic-book" };
		let html = Request::get(&url)?.html()?;
		let mut entries: Vec<Manga> = Vec::new();

		if let Some(items) = html.select(format!("div[class*='{}']", class)) {
			for element in items {
				let key = element
					.select_first("a")
					.and_then(|e| e.attr("href"))
					.unwrap_or_default()
					.split("/")
					.last()
					.unwrap_or_default()
					.to_string();
				let cover = handle_cover(
					element
						.select_first("a>img")
						.and_then(|e| e.attr("data-original"))
						.unwrap_or_default(),
				);
				let title = element
					.select_first("div>h2>a")
					.and_then(|e| e.text())
					.unwrap_or_default();
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
			let url = format!("{}/manhua/{}", WWW_URL, manga.key);
			let html = Request::get(&url)?.html()?;
			manga.cover = html
				.select_first(".comic-cover>img")
				.and_then(|e| e.attr("src"))
				.map(handle_cover);
			manga.title = html
				.select_first(".comic-title")
				.and_then(|e| e.text())
				.unwrap_or_default();
			let author_text = html
				.select_first("meta[property='og:novel:author']")
				.and_then(|e| e.attr("content"))
				.unwrap_or_default();
			let authors: Vec<String> = author_text
				.split(" ")
				.map(|a| a.trim().to_string())
				.filter(|a| !a.is_empty())
				.collect();
			manga.authors = Some(authors);
			manga.description = html
				.select_first(".comic_story")
				.and_then(|e| e.text());
			let cat_text = html
				.select_first("meta[property='og:novel:category']")
				.and_then(|e| e.attr("content"))
				.unwrap_or_default();
			manga.tags = Some(
				cat_text.split(" ").map(|a| a.to_string()).collect(),
			);
			manga.status = match html
				.select_first("meta[property='og:novel:status']")
				.and_then(|e| e.attr("content"))
				.unwrap_or_default()
				.as_str()
			{
				"连载中" => MangaStatus::Ongoing,
				"已完结" => MangaStatus::Completed,
				_ => MangaStatus::Unknown,
			};
			manga.content_rating = match html
				.select_first(".comic_age")
				.and_then(|e| e.text())
				.unwrap_or_default()
				.as_str()
			{
				"青年" | "女青" => ContentRating::Suggestive,
				_ => ContentRating::Safe,
			};
			manga.viewer = Viewer::RightToLeft;
			manga.url = Some(url);
		}

		if needs_chapters {
			let url = format!("{}/manhua/{}", WWW_URL, manga.key);
			let html = Request::get(&url)?.html()?;
			let mut chapters: Vec<Chapter> = Vec::new();

			if let Some(list) = html.select(".links-of-books>li>a") {
				for (index, element) in list.enumerate() {
					let chapter_id = element
						.attr("href")
						.unwrap_or_default()
						.split("/")
						.last()
						.unwrap_or_default()
						.replace(".html", "");
					let title = element.text().unwrap_or_default();
					chapters.push(Chapter {
						key: chapter_id.clone(),
						title: Some(title),
						chapter_number: Some((index + 1) as f32),
						url: Some(format!(
							"{}/manhua/{}/{}.html",
							WWW_URL, manga.key, chapter_id
						)),
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
		let url = format!(
			"{}/manhua/{}/{}.html",
			WWW_URL, manga.key, chapter.key
		);
		let text = Request::get(&url)?.string()?;
		let encoded = text
			.split_once("var img_data = ")
			.map(|(_, after)| after)
			.and_then(|s| s.split_once(";"))
			.map(|(before, _)| before.replace("'", ""))
			.unwrap_or_default();
		let data = general_purpose::STANDARD
			.decode(encoded.as_bytes())
			.unwrap_or_default();
		let data_str = core::str::from_utf8(&data).unwrap_or("[]");
		let list: Vec<PageImg> = serde_json::from_str(data_str).unwrap_or_default();
		let sub_path = chapter.key.replace("_", "/");
		let pages: Vec<Page> = list
			.into_iter()
			.map(|item| Page {
				content: PageContent::url(format!("{}/{}/{}", STATIC_URL, sub_path, item.img)),
				..Default::default()
			})
			.collect();
		Ok(pages)
	}
}

register_source!(ManhuadbSource);
