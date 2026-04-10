#![no_std]
use aidoku::{
	alloc::{String, Vec},
	helpers::uri::encode_uri,
	imports::{
		defaults::defaults_get,
		net::Request,
	},
	prelude::*,
	Chapter, ContentRating, FilterValue, Manga, MangaPageResult, MangaStatus, Page, PageContent,
	Result, Source, Viewer,
};
use aidoku::alloc::string::ToString;

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36";

fn get_url() -> String {
	defaults_get::<String>("url").unwrap_or_else(|| String::from("https://www.jjmh.top"))
}

struct MxshmSource;

impl Source for MxshmSource {
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
		let mut area = String::from("-1");
		let mut end = String::from("-1");

		for filter in filters {
			if let FilterValue::Select { id, value } = filter {
				match id.as_str() {
					"tag" => tag = value,
					"area" => area = value,
					"end" => end = value,
					_ => {}
				}
			}
		}

		let base = get_url();
		let url = if let Some(query) = query {
			format!("{}/search?keyword={}", base, encode_uri(query))
		} else {
			format!(
				"{}/booklist?tag={}&area={}&end={}&page={}",
				base,
				encode_uri(tag),
				area,
				end,
				page
			)
		};
		let html = Request::get(&url)?.header("User-Agent", UA).html()?;
		let mut entries: Vec<Manga> = Vec::new();

		if let Some(items) = html.select(".mh-item") {
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
				let cover = item
					.select_first("a>p")
					.and_then(|e| e.attr("style"))
					.unwrap_or_default()
					.replace("background-image: url(", "")
					.replace(")", "");
				let title = item
					.select_first(".mh-item-detali>h2>a")
					.and_then(|e| e.text())
					.unwrap_or_default()
					.trim()
					.to_string();
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
		let base = get_url();

		if needs_details {
			let url = format!("{}/book/{}", base, manga.key);
			let html = Request::get(&url)?.header("User-Agent", UA).html()?;

			manga.cover = html
				.select_first(".banner_detail_form>.cover>img")
				.and_then(|e| e.attr("src"));
			manga.title = html
				.select_first(".banner_detail_form>.info>h1")
				.and_then(|e| e.text())
				.unwrap_or_default()
				.trim()
				.to_string();
			let author_text = html
				.select_first(".banner_detail_form>.info>p:nth-child(3)")
				.and_then(|e| e.text())
				.unwrap_or_default()
				.trim()
				.replace("作者：", "");
			let authors: Vec<String> = author_text
				.split("&")
				.map(|a| a.trim().to_string())
				.filter(|a| !a.is_empty())
				.collect();
			manga.authors = Some(authors);
			manga.description = html
				.select_first(".banner_detail_form>.info>.content")
				.and_then(|e| e.text())
				.map(|s| s.trim().to_string());

			let mut tags = Vec::new();
			if let Some(items) = html.select(".banner_detail_form>.info>p:nth-child(5)>span>a") {
				for item in items {
					if let Some(t) = item.text() {
						let t = t.trim().to_string();
						if !t.is_empty() {
							tags.push(t);
						}
					}
				}
			}
			manga.tags = Some(tags);

			let status_text = html
				.select_first(".banner_detail_form>.info>p:nth-child(4)>span:nth-child(1)>span")
				.and_then(|e| e.text())
				.unwrap_or_default();
			manga.status = match status_text.trim() {
				"连载中" => MangaStatus::Ongoing,
				"已完结" => MangaStatus::Completed,
				_ => MangaStatus::Unknown,
			};
			manga.content_rating = ContentRating::NSFW;
			manga.viewer = Viewer::Webtoon;
			manga.url = Some(url);
		}

		if needs_chapters {
			let url = format!("{}/book/{}", base, manga.key);
			let html = Request::get(&url)?.header("User-Agent", UA).html()?;
			let mut chapters: Vec<Chapter> = Vec::new();

			if let Some(items) = html.select("#detail-list-select>li>a") {
				for (index, item) in items.enumerate() {
					let key = item
						.attr("href")
						.unwrap_or_default()
						.split("/")
						.map(|a| a.to_string())
						.collect::<Vec<String>>()
						.pop()
						.unwrap_or_default();
					let title = item.text().map(|s| s.trim().to_string());
					let chapter_url = format!("{}/chapter/{}", base, key);
					chapters.push(Chapter {
						key,
						title,
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

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{}/chapter/{}", get_url(), chapter.key);
		let html = Request::get(&url)?.header("User-Agent", UA).html()?;
		let mut pages: Vec<Page> = Vec::new();

		if let Some(items) = html.select(".comicpage>div>img,#cp_img>img") {
			for item in items {
				let img_url = item.attr("data-original").unwrap_or_default().trim().to_string();
				pages.push(Page {
					content: PageContent::url(img_url),
					..Default::default()
				});
			}
		}

		Ok(pages)
	}
}

register_source!(MxshmSource);
