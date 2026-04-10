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
use serde::Deserialize;

mod helper;

const FILTER_CATEGORY_ID: [&str; 15] = [
	"", "1", "15", "32", "6", "13", "28", "31", "22", "23", "26", "29", "34", "35", "36",
];
const FILTER_JINDU: [&str; 3] = ["", "0", "1"];
const FILTER_SHUXING: [&str; 4] = ["", "一半中文一半生肉", "全生肉", "全中文"];
const FILTER_AREA: [&str; 2] = ["", "日本"];
const FILTER_ODFIE: [&str; 2] = ["addtime", "edittime"];

#[derive(Deserialize)]
struct PageImage {
	file: String,
}

struct ZerobywnsSource;

impl Source for ZerobywnsSource {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut category_id = String::new();
		let mut jindu = String::new();
		let mut shuxing = String::new();
		let mut area = String::new();
		let mut odfie = String::from("addtime");
		let mut order = String::from("desc");

		for filter in filters {
			match filter {
				FilterValue::Select { id, value } => match id.as_str() {
					"category" => {
						if let Ok(index) = value.parse::<usize>() {
							if let Some(s) = FILTER_CATEGORY_ID.get(index) {
								category_id = s.to_string();
							}
						}
					}
					"jindu" => {
						if let Ok(index) = value.parse::<usize>() {
							if let Some(s) = FILTER_JINDU.get(index) {
								jindu = s.to_string();
							}
						}
					}
					"shuxing" => {
						if let Ok(index) = value.parse::<usize>() {
							if let Some(s) = FILTER_SHUXING.get(index) {
								shuxing = s.to_string();
							}
						}
					}
					"area" => {
						if let Ok(index) = value.parse::<usize>() {
							if let Some(s) = FILTER_AREA.get(index) {
								area = s.to_string();
							}
						}
					}
					_ => {}
				},
				FilterValue::Sort { id, index, ascending } => {
					if id == "sort" {
						if let Some(s) = FILTER_ODFIE.get(index as usize) {
							odfie = s.to_string();
						}
						if ascending {
							order = String::from("asc");
						}
					}
				}
				_ => {}
			}
		}

		let mut entries: Vec<Manga> = Vec::new();

		if let Some(query) = query {
			let url = format!(
				"{}/plugin.php?id=jameson_manhua&c=index&a=search&keyword={}&page={}",
				helper::get_url(),
				encode_uri(query),
				page
			);
			let html = helper::get_html(&url)?;

			if let Some(items) = html.select(".uk-card") {
				for item in items {
					let key = item
						.attr("href")
						.unwrap_or_default()
						.split("=")
						.map(|a| a.to_string())
						.collect::<Vec<String>>()
						.pop()
						.unwrap_or_default();
					let cover = item
						.select_first("div:nth-child(1)>img")
						.and_then(|e| e.attr("src"));
					let title = item
						.select_first("div:nth-child(2)>p")
						.and_then(|e| e.text())
						.unwrap_or_default()
						.trim()
						.to_string();
					entries.push(Manga {
						key,
						cover,
						title,
						..Default::default()
					});
				}
			}
		} else {
			let mut url = format!(
				"{}/plugin.php?id=jameson_manhua&c=index&a=ku",
				helper::get_url()
			);
			if !category_id.is_empty() {
				url.push_str(&format!("&category_id={}", category_id));
			}
			if !jindu.is_empty() {
				url.push_str(&format!("&jindu={}", jindu));
			}
			if !shuxing.is_empty() {
				url.push_str(&format!("&shuxing={}", encode_uri(shuxing)));
			}
			if !area.is_empty() {
				url.push_str(&format!("&area={}", encode_uri(area)));
			}
			url.push_str(&format!("&odfie={}&order={}&page={}", odfie, order, page));

			let html = helper::get_html(&url)?;

			if let Some(items) = html.select(".uk-card") {
				for item in items {
					let key = item
						.select_first("div:nth-child(1)>a")
						.and_then(|e| e.attr("href"))
						.unwrap_or_default()
						.split("=")
						.map(|a| a.to_string())
						.collect::<Vec<String>>()
						.pop()
						.unwrap_or_default();
					let cover = item
						.select_first("div:nth-child(1)>a>img")
						.and_then(|e| e.attr("src"));
					let title = item
						.select_first("div:nth-child(2)>p>a")
						.and_then(|e| e.text())
						.unwrap_or_default()
						.trim()
						.to_string();
					entries.push(Manga {
						key,
						cover,
						title,
						..Default::default()
					});
				}
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
		let url = format!(
			"{}/plugin.php?id=jameson_manhua&c=index&a=bofang&kuid={}",
			helper::get_url(),
			manga.key
		);

		if needs_details {
			let html = helper::get_html(&url)?;

			manga.cover = html
				.select_first(".uk-width-medium>img")
				.and_then(|e| e.attr("src"));
			manga.title = html
				.select_first(".uk-margin-left>ul>li>h3")
				.and_then(|e| e.text())
				.unwrap_or_default();

			let author_text = html
				.select_first(".uk-margin-left>ul>li>.cl>a[href*='zuozhe']")
				.and_then(|e| e.text())
				.unwrap_or_default()
				.replace("作者:", "");
			let authors: Vec<String> = author_text
				.split("×")
				.map(|a| a.to_string())
				.collect();
			manga.authors = Some(authors);

			manga.description = html
				.select_first(".uk-margin-left>ul>li>.uk-alert")
				.and_then(|e| e.text())
				.map(|s| s.trim().to_string());

			let mut tags = Vec::new();
			if let Some(items) = html.select(".uk-margin-left>ul>li>.cl>a[href*='category']") {
				for item in items {
					if let Some(t) = item.text() {
						tags.push(t);
					}
				}
			}
			manga.tags = Some(tags);

			manga.status = match html
				.select_first(".uk-margin-left>ul>li>.cl>span:nth-child(6)")
				.and_then(|e| e.text())
				.unwrap_or_default()
				.as_str()
			{
				"连载中" => MangaStatus::Ongoing,
				"已完结" => MangaStatus::Completed,
				_ => MangaStatus::Unknown,
			};
			manga.content_rating = ContentRating::Safe;
			manga.viewer = Viewer::RightToLeft;
			manga.url = Some(url.clone());
		}

		if needs_chapters {
			let html = helper::get_html(&url)?;
			let mut chapters: Vec<Chapter> = Vec::new();

			if let Some(items) = html.select(".muludiv>a") {
				for (index, item) in items.enumerate() {
					let chapter_key = item
						.attr("href")
						.unwrap_or_default()
						.split("=")
						.map(|a| a.to_string())
						.collect::<Vec<String>>()
						.pop()
						.unwrap_or_default();
					let title = item.text().unwrap_or_default();
					let chapter_url = format!(
						"{}/plugin.php?id=jameson_manhua&a=read&zjid={}",
						helper::get_url(),
						chapter_key
					);
					chapters.push(Chapter {
						key: chapter_key,
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

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!(
			"{}/plugin.php?id=jameson_manhua&a=read&zjid={}",
			helper::get_url(),
			chapter.key
		);
		let html = helper::get_html(&url)?;
		let text = html
			.select_first("html")
			.and_then(|e| e.html())
			.unwrap_or_default();
		let list_str = text
			.split_once("let listimg=")
			.and_then(|(_, after)| after.split_once(";"))
			.map(|(before, _)| before)
			.unwrap_or("[]");
		let list: Vec<PageImage> = serde_json::from_str(list_str).unwrap_or_default();
		let pages: Vec<Page> = list
			.into_iter()
			.map(|item| Page {
				content: PageContent::url(item.file),
				..Default::default()
			})
			.collect();

		Ok(pages)
	}
}

impl ImageRequestProvider for ZerobywnsSource {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		Ok(Request::get(&url)?.header("Referer", &helper::get_url()))
	}
}

register_source!(ZerobywnsSource, ImageRequestProvider);
