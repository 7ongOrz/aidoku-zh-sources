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
use regex::Regex;
use serde::Deserialize;

const WWW_URL: &str = "https://mycomic.com/cn";

const FILTER_SORT: [&str; 3] = ["", "update", "views"];

#[derive(Deserialize)]
struct ChapterJson {
	id: String,
	title: String,
}

fn extract_chapter_number(title: &str) -> Option<f32> {
	let re = Regex::new(
		r"(?:第\s*)(\d+(?:\.\d+)?)|(\d+(?:\.\d+)?)\s*(?:话|話|章|回|卷|册|冊)",
	)
	.unwrap();
	if let Some(captures) = re.captures(title) {
		let num_match = captures.get(1).or_else(|| captures.get(2));
		if let Some(num_match) = num_match {
			if let Ok(num) = num_match.as_str().parse::<f32>() {
				return Some(num);
			}
		}
	}
	None
}

struct MycomicSource;

impl Source for MycomicSource {
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
		let mut country = String::new();
		let mut audience = String::new();
		let mut year = String::new();
		let mut end = String::new();
		let mut sort = String::new();

		for filter in filters {
			match filter {
				FilterValue::Select { id, value } => match id.as_str() {
					"tag" => tag = value,
					"country" => country = value,
					"audience" => audience = value,
					"year" => year = value,
					"end" => end = value,
					_ => {}
				},
				FilterValue::Sort {
					id, index, ascending,
				} => {
					if id == "sort" {
						if let Some(s) = FILTER_SORT.get(index as usize) {
							sort = s.to_string();
						}
						if sort.is_empty() && ascending {
							sort = String::from("time");
						} else if !sort.is_empty() && !ascending {
							sort = format!("-{}", sort);
						}
					}
				}
				_ => {}
			}
		}

		let url = if let Some(query) = query {
			format!(
				"{}/comics?q={}&page={}",
				WWW_URL,
				encode_uri(query),
				page
			)
		} else {
			format!(
				"{}/comics?filter[tag]={}&filter[country]={}&filter[audience]={}&filter[year]={}&filter[end]={}&sort={}&page={}",
				WWW_URL, tag, country, audience, year, end, sort, page
			)
		};

		let html = Request::get(&url)?.html()?;
		let mut entries: Vec<Manga> = Vec::new();

		if let Some(items) = html.select(".group") {
			for item in items {
				let key = item
					.select_first("a")
					.and_then(|e| e.attr("href"))
					.unwrap_or_default()
					.split('/')
					.filter(|a| !a.is_empty())
					.map(|a| a.to_string())
					.collect::<Vec<String>>()
					.pop()
					.unwrap_or_default();
				let img = item.select_first("a>img");
				let cover = img
					.as_ref()
					.and_then(|e| e.attr("data-src"))
					.or_else(|| img.as_ref().and_then(|e| e.attr("src")));
				let title = img
					.as_ref()
					.and_then(|e| e.attr("alt"))
					.unwrap_or_default();
				entries.push(Manga {
					key,
					title,
					cover,
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
		let url = format!("{}/comics/{}", WWW_URL, manga.key);
		let html = Request::get(&url)?.html()?;

		if needs_details {
			let cdn_url = html
				.select_first("body[x-data]")
				.and_then(|e| e.attr("x-data"))
				.unwrap_or_default();
			let cdn_base = cdn_url
				.split_once("cdnUrl: '")
				.and_then(|(_, after)| after.split_once("'"))
				.map(|(before, _)| format!("https://{}", before))
				.unwrap_or_else(|| String::from("https://biccam.com"));

			let mut cover = html
				.select_first("meta[name='og:image']")
				.and_then(|e| e.attr("content"))
				.unwrap_or_default();
			if cover.is_empty() {
				cover = html
					.select_first("img.object-cover")
					.and_then(|e| e.attr("src"))
					.unwrap_or_default();
			}
			if !cover.is_empty() && !cover.starts_with("http") {
				if cover.starts_with("//") {
					cover = format!("https:{}", cover);
				} else if cover.starts_with('/') {
					cover = format!("{}{}", cdn_base, cover);
				}
			}
			manga.cover = Some(cover);

			manga.title = html
				.select_first("title")
				.and_then(|e| e.text())
				.unwrap_or_default()
				.replace(" - MYCOMIC - 我的漫画", "");

			let author = html
				.select_first("meta[name='author']")
				.and_then(|e| e.attr("content"))
				.unwrap_or_default();
			manga.authors = Some(aidoku::alloc::vec![author]);

			let mut description = html
				.select_first("div[x-show='show']")
				.and_then(|e| e.text())
				.map(|s| s.trim().to_string())
				.unwrap_or_default();
			if description.is_empty() {
				description = html
					.select_first("meta[name='description']")
					.and_then(|e| e.attr("content"))
					.map(|s| s.trim().to_string())
					.unwrap_or_default();
			}
			manga.description = Some(description);

			let mut tags = Vec::new();
			if let Some(tag_elements) = html.select("a[href*='tag']") {
				for el in tag_elements {
					if let Some(t) = el.text() {
						tags.push(t);
					}
				}
			}
			manga.tags = Some(tags);

			manga.status = html
				.select_first("div[data-flux-badge]")
				.and_then(|e| e.text())
				.map(|s| match s.trim() {
					"连载中" => MangaStatus::Ongoing,
					"已完结" => MangaStatus::Completed,
					_ => MangaStatus::Unknown,
				})
				.unwrap_or(MangaStatus::Unknown);
			manga.content_rating = ContentRating::Safe;
			manga.viewer = Viewer::RightToLeft;
			manga.url = Some(url);
		}

		if needs_chapters {
			let mut all_chapters: Vec<Chapter> = Vec::new();

			if let Some(elements) = html.select("div[x-data*='chapters']") {
				for element in elements {
					let scanlator = element
						.select_first("div[data-flux-subheading] div")
						.and_then(|e| e.text())
						.map(|s| s.trim().to_string())
						.unwrap_or_default();

					let data = element.attr("x-data").unwrap_or_default();
					let text = data
						.split_once("chapters:")
						.and_then(|(_, after)| after.split_once("],"))
						.map(|(before, _)| {
							let mut s = before.trim().to_string();
							s.push(']');
							s
						});

					let text = match text {
						Some(t) => t,
						None => continue,
					};

					let list: Vec<ChapterJson> = match serde_json::from_str(&text) {
						Ok(l) => l,
						Err(_) => continue,
					};
					let len = list.len();

					for (index, item) in list.iter().enumerate() {
						let chapter_num = (len - index) as f32;
						let chapter_or_volume =
							extract_chapter_number(&item.title).unwrap_or(chapter_num);
						let (ch, vo) = if item.title.trim().ends_with('卷') {
							(-1.0, chapter_or_volume)
						} else {
							(chapter_or_volume, -1.0)
						};

						all_chapters.push(Chapter {
							key: item.id.clone(),
							title: Some(item.title.clone()),
							volume_number: Some(vo),
							chapter_number: Some(ch),
							url: Some(format!("{}/chapters/{}", WWW_URL, item.id)),
							scanlators: Some(aidoku::alloc::vec![scanlator.clone()]),
							..Default::default()
						});
					}
				}
			}

			manga.chapters = Some(all_chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{}/chapters/{}", WWW_URL, chapter.key);
		let html = Request::get(&url)?.html()?;
		let mut pages: Vec<Page> = Vec::new();

		if let Some(items) = html.select("img.page") {
			for item in items {
				let img_url = if item.has_attr("data-src") {
					item.attr("data-src").unwrap_or_default()
				} else {
					item.attr("src").unwrap_or_default()
				};
				pages.push(Page {
					content: PageContent::url(img_url),
					..Default::default()
				});
			}
		}

		Ok(pages)
	}
}

impl ImageRequestProvider for MycomicSource {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		Ok(Request::get(&url)?.header("Referer", WWW_URL))
	}
}

register_source!(MycomicSource, ImageRequestProvider);
