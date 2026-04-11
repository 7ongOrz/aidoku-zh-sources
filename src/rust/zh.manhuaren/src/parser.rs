use aidoku::{
	alloc::{String, Vec},
	imports::html::Document,
	prelude::*,
	Chapter, ContentRating, Manga, MangaStatus, Page, PageContent, Viewer,
};
use aidoku::alloc::string::ToString;
use serde::Deserialize;

use crate::helper;

// dm5.ashx listing response
#[derive(Deserialize)]
pub struct ListingResponse {
	#[serde(rename = "UpdateComicItems", default)]
	pub items: Vec<ComicItem>,
}

#[derive(Deserialize)]
pub struct ComicItem {
	#[serde(rename = "Title", default)]
	pub title: String,
	#[serde(rename = "UrlKey", default)]
	pub url_key: String,
	#[serde(rename = "ShowPicUrlB", default)]
	pub pic_url: String,
	#[serde(rename = "ShowConver", default)]
	pub cover_url: String,
	#[serde(rename = "Author", default)]
	pub author: Vec<String>,
	#[serde(rename = "Content", default)]
	pub content: String,
	#[serde(rename = "Status", default)]
	pub status: i32,
}

// search.ashx response
#[derive(Deserialize)]
pub struct SearchItem {
	#[serde(rename = "Title", default)]
	pub title: String,
	#[serde(rename = "Url", default)]
	pub url: String,
}

pub fn parse_listing(items: &[ComicItem]) -> Vec<Manga> {
	items
		.iter()
		.map(|item| {
			let cover = if item.pic_url.is_empty() {
				item.cover_url.clone()
			} else {
				item.pic_url.clone()
			};
			Manga {
				key: item.url_key.clone(),
				title: item.title.clone(),
				cover: Some(cover),
				authors: Some(item.author.clone()),
				description: if item.content.is_empty() {
					None
				} else {
					Some(item.content.clone())
				},
				status: match item.status {
					1 => MangaStatus::Completed,
					_ => MangaStatus::Ongoing,
				},
				content_rating: ContentRating::Safe,
				viewer: Viewer::RightToLeft,
				..Default::default()
			}
		})
		.collect()
}

pub fn parse_search(items: &[SearchItem]) -> Vec<Manga> {
	items
		.iter()
		.map(|item| Manga {
			key: item.url.clone(),
			title: item.title.clone(),
			..Default::default()
		})
		.collect()
}

pub fn parse_manga_detail(html: &Document) -> Manga {
	let title = html
		.select_first(".detail-main-info-title")
		.and_then(|e| e.text())
		.unwrap_or_default();

	let cover = html
		.select_first(".detail-main-cover img")
		.and_then(|e| e.attr("src"));

	let authors: Option<Vec<String>> = html
		.select(".detail-main-info-author a")
		.map(|list| list.filter_map(|a| a.text()).collect());

	let tags: Option<Vec<String>> = html
		.select(".detail-main-info-class a")
		.map(|list| list.filter_map(|a| a.text()).collect());

	let description = html.select_first(".detail-desc").and_then(|e| e.text());

	let status = html
		.select_first(".detail-list-title-1")
		.and_then(|e| e.text())
		.map(|s| {
			if s.contains("完结") {
				MangaStatus::Completed
			} else {
				MangaStatus::Ongoing
			}
		})
		.unwrap_or(MangaStatus::Unknown);

	Manga {
		key: String::new(),
		title,
		cover,
		authors,
		description,
		url: None, // filled by get_manga_update from manga_url(slug)
		tags,
		status,
		content_rating: ContentRating::Safe,
		viewer: Viewer::RightToLeft,
		..Default::default()
	}
}

pub fn parse_chapter_list(html: &Document) -> Vec<Chapter> {
	let mut chapters = Vec::new();
	let items = match html.select("a.chapteritem") {
		Some(list) => list,
		None => return chapters,
	};
	for item in items {
		let href = item.attr("href").unwrap_or_default();
		let key = href
			.trim_matches('/')
			.split('/')
			.filter(|s| !s.is_empty())
			.last()
			.unwrap_or_default()
			.to_string();
		if key.is_empty() {
			continue;
		}

		let name = item.text().unwrap_or_default();
		let subtitle = item.attr("title").unwrap_or_default();
		let title = if subtitle.is_empty() {
			name.clone()
		} else {
			format!("{} {}", name, subtitle)
		};

		chapters.push(Chapter {
			key: key.clone(),
			title: Some(title),
			chapter_number: extract_number(&name),
			url: Some(helper::chapter_url(&key)),
			..Default::default()
		});
	}
	chapters
}

pub fn parse_page_list(html_text: &str) -> Vec<Page> {
	helper::unpack_images(html_text)
		.into_iter()
		.map(|url| Page {
			content: PageContent::url(url),
			..Default::default()
		})
		.collect()
}

fn extract_number(text: &str) -> Option<f32> {
	let mut num = String::new();
	let mut found = false;
	for ch in text.chars() {
		if ch.is_ascii_digit() || (ch == '.' && found) {
			num.push(ch);
			found = true;
		} else if found {
			break;
		}
	}
	if found { num.parse().ok() } else { None }
}
