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
use serde::Deserialize;

const WWW_URL: &str = "https://se8.us/index.php";
const UA: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 16_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.6 Mobile/15E148 Safari/604.1";

const FILTER_ORDER: [&str; 2] = ["hits", "addtime"];

#[derive(Deserialize)]
struct ChapterResponse {
	data: Vec<ChapterData>,
}

#[derive(Deserialize)]
struct ChapterData {
	id: String,
	name: String,
	link: String,
}

struct Se8Source;

impl Source for Se8Source {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut tag = String::new();
		let mut finish = String::new();
		let mut order = String::from("hits");

		for filter in filters {
			match filter {
				FilterValue::Select { id, value } => match id.as_str() {
					"tag" => tag = value,
					"finish" => finish = value,
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

		let is_search = query.is_some();
		let url = if let Some(query) = query {
			format!("{}/search/{}/{}", WWW_URL, encode_uri(query), page)
		} else {
			let mut url = format!("{}/category", WWW_URL);
			if !tag.is_empty() {
				url.push_str(&format!("/tags/{}", tag));
			}
			if !finish.is_empty() {
				url.push_str(&format!("/finish/{}", finish));
			}
			format!("{}/order/{}/page/{}", url, order, page)
		};

		let html = Request::get(&url)?.header("User-Agent", UA).html()?;
		let mut entries: Vec<Manga> = Vec::new();

		if let Some(items) = html.select(".comic-item,.comic-list-item") {
			for item in items {
				let key = item
					.select_first("a")
					.and_then(|e| e.attr("href"))
					.unwrap_or_default()
					.split("/")
					.map(|a| a.to_string())
					.collect::<Vec<String>>()
					.pop()
					.unwrap_or_default();
				let cover = if is_search {
					item.select_first("a>img")
						.and_then(|e| e.attr("src"))
				} else {
					item.select_first("a>img")
						.and_then(|e| e.attr("data-src"))
				};
				let title = item
					.select_first("div>.comic-name")
					.and_then(|e| e.text())
					.unwrap_or_default();
				entries.push(Manga {
					key,
					cover,
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
			let url = format!("{}/comic/{}", WWW_URL, manga.key);
			let html = Request::get(&url)?.header("User-Agent", UA).html()?;

			// extract comic ID from inline script
			let page_html = html.select_first("html")
				.and_then(|e| e.html())
				.unwrap_or_default();
			let comic_id = page_html
				.split_once("comic.show(")
				.and_then(|(_, after)| after.split_once(");"))
				.map(|(before, _)| before.to_string())
				.unwrap_or_default();
			manga.key = comic_id;

			manga.cover = html
				.select_first(".comic-info-box>.box-back")
				.and_then(|e| e.attr("style"))
				.map(|s| {
					s.replace("background-image: url('", "")
						.replace("')", "")
				});
			manga.title = html
				.select_first(".comic-info-box>.comic-info>h1")
				.and_then(|e| e.text())
				.unwrap_or_default();
			let author_text = html
				.select_first(".comic-info-box>.comic-info>.au-name")
				.and_then(|e| e.text())
				.unwrap_or_default()
				.replace("作者：", "")
				.replace("&amp", "&");
			let authors: Vec<String> = author_text
				.split("&")
				.filter(|a| !a.trim().is_empty())
				.map(|a| a.trim().to_string())
				.collect();
			manga.authors = Some(authors);
			manga.description = html
				.select_first(".comic-intro")
				.and_then(|e| e.text())
				.map(|s| s.trim().replace("&hellip", "…"));
			let tags_html = html
				.select_first(".comic-info-box>.comic-info>.comic-tags")
				.and_then(|e| e.html())
				.unwrap_or_default();
			let tags: Vec<String> = tags_html
				.split_once("</span>")
				.map(|(_, after)| after)
				.unwrap_or("")
				.trim()
				.split(" ")
				.filter(|a| !a.trim().is_empty())
				.map(|a| a.to_string())
				.collect();
			manga.tags = Some(tags);
			manga.status = MangaStatus::Ongoing;
			manga.content_rating = ContentRating::NSFW;
			manga.viewer = Viewer::Webtoon;
			manga.url = Some(url);
		}

		if needs_chapters {
			let url = format!("{}/api/comic/chapter?mid={}", WWW_URL, manga.key);
			let resp: ChapterResponse = Request::get(&url)?
				.header("User-Agent", UA)
				.json_owned()?;
			let mut chapters: Vec<Chapter> = Vec::new();

			for (index, item) in resp.data.iter().enumerate() {
				let title = item
					.name
					.trim()
					.replace("&lt;", "<")
					.replace("&gt;", ">")
					.replace("&#40;", "(")
					.replace("&#41;", ")")
					.replace("&ldquo;", "\u{201c}")
					.replace("&rdquo;", "\u{201d}")
					.replace("&hellip;", "…")
					.replace("&hearts;", "♥");
				chapters.push(Chapter {
					key: item.id.clone(),
					title: Some(title),
					chapter_number: Some((index + 1) as f32),
					url: Some(item.link.clone()),
					..Default::default()
				});
			}
			chapters.reverse();
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{}/chapter/{}", WWW_URL, chapter.key);
		let html = Request::get(&url)?.header("User-Agent", UA).html()?;
		let mut pages: Vec<Page> = Vec::new();

		if let Some(items) = html.select(".comic-page>img") {
			for item in items {
				let img_url = item.attr("src").unwrap_or_default().trim().to_string();
				pages.push(Page {
					content: PageContent::url(img_url),
					..Default::default()
				});
			}
		}

		Ok(pages)
	}
}

register_source!(Se8Source);
