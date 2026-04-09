use aidoku::{
	alloc::{String, Vec},
	prelude::*,
	Chapter, ContentRating, Manga, MangaPageResult, MangaStatus, Page, PageContent, Viewer,
};
use aidoku::alloc::string::ToString;
use serde::Deserialize;

use crate::helper;

#[derive(Deserialize)]
pub struct PicaResponse<T> {
	pub data: T,
}

#[derive(Deserialize)]
pub struct ComicsContainer<T> {
	pub comics: T,
}

#[derive(Deserialize)]
pub struct PagedList<T> {
	pub docs: Vec<T>,
	pub page: i32,
	pub pages: i32,
	#[serde(default)]
	pub limit: i32,
}

#[derive(Deserialize)]
pub struct MangaItem {
	#[serde(rename = "_id")]
	pub id: String,
	pub title: String,
	#[serde(default)]
	pub author: String,
	#[serde(default)]
	pub description: String,
	#[serde(default)]
	pub categories: Vec<String>,
	#[serde(default)]
	pub finished: bool,
	pub thumb: Thumb,
}

#[derive(Deserialize)]
pub struct Thumb {
	#[serde(rename = "fileServer")]
	pub file_server: String,
	pub path: String,
}

#[derive(Deserialize)]
pub struct EpsResponse {
	pub eps: PagedList<EpsItem>,
}

#[derive(Deserialize)]
pub struct EpsItem {
	pub order: i32,
	pub title: String,
}

#[derive(Deserialize)]
pub struct PagesResponse {
	pub pages: PagedList<PageItem>,
}

#[derive(Deserialize)]
pub struct PageItem {
	pub media: Thumb,
}

pub fn manga_from_item(item: &MangaItem) -> Manga {
	let cover = format!("{}/static/{}", item.thumb.file_server, item.thumb.path);
	let url = helper::gen_manga_url(item.id.clone());
	let authors: Vec<String> = item
		.author
		.split("&")
		.map(|a| a.trim().to_string())
		.filter(|a| !a.is_empty())
		.collect();
	let viewer = if item.categories.contains(&String::from("WEBTOON")) {
		Viewer::Webtoon
	} else {
		Viewer::RightToLeft
	};
	Manga {
		key: item.id.clone(),
		title: item.title.clone(),
		cover: Some(cover),
		authors: Some(authors),
		description: Some(item.description.clone()),
		url: Some(url),
		tags: Some(item.categories.clone()),
		status: if item.finished {
			MangaStatus::Completed
		} else {
			MangaStatus::Ongoing
		},
		content_rating: ContentRating::NSFW,
		viewer,
		..Default::default()
	}
}

pub fn parse_manga_list(items: &[MangaItem]) -> Vec<Manga> {
	items.iter().map(manga_from_item).collect()
}

pub fn parse_paged_list<T>(list: &PagedList<T>) -> bool {
	list.pages > list.page
}

pub fn parse_chapter_list(manga_id: &str, items: &[EpsItem]) -> Vec<Chapter> {
	items
		.iter()
		.map(|item| {
			let key = item.order.to_string();
			let url = helper::gen_chapter_url(manga_id.to_string(), key.clone());
			Chapter {
				key,
				title: Some(item.title.clone()),
				chapter_number: Some(item.order as f32),
				url: Some(url),
				..Default::default()
			}
		})
		.collect()
}

pub fn parse_page_list(items: &[PageItem], _offset: i32) -> Vec<Page> {
	items
		.iter()
		.map(|item| Page {
			content: PageContent::url(format!(
				"{}/static/{}",
				item.media.file_server, item.media.path
			)),
			..Default::default()
		})
		.collect()
}

pub fn build_result<T>(items: &[MangaItem], paged: &PagedList<T>) -> MangaPageResult {
	MangaPageResult {
		entries: parse_manga_list(items),
		has_next_page: parse_paged_list(paged),
	}
}
