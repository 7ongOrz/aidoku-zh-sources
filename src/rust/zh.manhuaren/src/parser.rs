use aidoku::{
	alloc::{String, Vec},
	Chapter, ContentRating, Manga, MangaStatus, Page, PageContent, Viewer,
};
use aidoku::alloc::string::ToString;
use serde::Deserialize;

use crate::helper;

#[derive(Deserialize)]
pub struct ApiResponse {
	pub response: ResponseData,
}

#[derive(Deserialize)]
pub struct ResponseData {
	#[serde(default)]
	pub mangas: Vec<MangaItem>,
	#[serde(default)]
	pub result: Vec<MangaItem>,
	#[serde(default)]
	pub total: i32,
	// Detail fields
	#[serde(rename = "mangaId", default)]
	pub manga_id: String,
	#[serde(rename = "mangaName", default)]
	pub manga_name: String,
	#[serde(rename = "mangaCoverimageUrl", default)]
	pub manga_cover_image_url: String,
	#[serde(rename = "mangaPicimageUrl", default)]
	pub manga_pic_image_url: String,
	#[serde(rename = "mangaAuthor", default)]
	pub manga_author: String,
	#[serde(rename = "mangaIntro", default)]
	pub manga_intro: String,
	#[serde(rename = "shareUrl", default)]
	pub share_url: String,
	#[serde(rename = "mangaTheme", default)]
	pub manga_theme: String,
	#[serde(rename = "mangaIsOver", default)]
	pub manga_is_over: i32,
	#[serde(rename = "mangaWords", default)]
	pub manga_words: Vec<ChapterItem>,
	#[serde(rename = "mangaRolls", default)]
	pub manga_rolls: Vec<ChapterItem>,
	#[serde(rename = "mangaEpisode", default)]
	pub manga_episode: Vec<ChapterItem>,
}

#[derive(Deserialize)]
pub struct MangaItem {
	#[serde(rename = "mangaId")]
	pub manga_id: String,
	#[serde(rename = "mangaName")]
	pub manga_name: String,
	#[serde(rename = "mangaCoverimageUrl", default)]
	pub manga_cover_image_url: String,
	#[serde(rename = "mangaPicimageUrl", default)]
	pub manga_pic_image_url: String,
	#[serde(rename = "mangaAuthor", default)]
	pub manga_author: String,
	#[serde(rename = "mangaIntro", default)]
	pub manga_intro: String,
	#[serde(rename = "shareUrl", default)]
	pub share_url: String,
	#[serde(rename = "mangaTheme", default)]
	pub manga_theme: String,
	#[serde(rename = "mangaIsOver", default)]
	pub manga_is_over: i32,
}

#[derive(Deserialize)]
pub struct ChapterItem {
	#[serde(rename = "sectionId")]
	pub section_id: String,
	#[serde(rename = "sectionTitle", default)]
	pub section_title: String,
	#[serde(rename = "sectionName", default)]
	pub section_name: String,
	#[serde(rename = "isMustPay", default)]
	pub is_must_pay: i32,
	#[serde(rename = "sectionSort", default)]
	pub section_sort: f32,
}

#[derive(Deserialize)]
pub struct PageResponse {
	pub response: PageData,
}

#[derive(Deserialize)]
pub struct PageData {
	#[serde(rename = "mangaSectionImages", default)]
	pub manga_section_images: Vec<String>,
	#[serde(rename = "hostList", default)]
	pub host_list: Vec<String>,
	#[serde(default)]
	pub query: String,
}

pub fn parse_manga_list(items: &[MangaItem]) -> Vec<Manga> {
	items.iter().map(parse_manga_item).collect()
}

fn parse_manga_item(item: &MangaItem) -> Manga {
	let cover = if item.manga_pic_image_url.is_empty() {
		item.manga_cover_image_url.clone()
	} else {
		item.manga_pic_image_url.clone()
	};
	let categories: Vec<String> = item
		.manga_theme
		.split(" ")
		.filter(|c| !c.is_empty())
		.map(|c| c.to_string())
		.collect();
	let status = match item.manga_is_over {
		0 => MangaStatus::Ongoing,
		1 => MangaStatus::Completed,
		_ => MangaStatus::Unknown,
	};
	Manga {
		key: item.manga_id.clone(),
		title: item.manga_name.clone(),
		cover: Some(cover),
		authors: Some(aidoku::alloc::vec![item.manga_author.trim().to_string()]),
		description: Some(item.manga_intro.clone()),
		url: Some(item.share_url.clone()),
		tags: Some(categories),
		status,
		content_rating: ContentRating::Safe,
		viewer: Viewer::RightToLeft,
		..Default::default()
	}
}

pub fn parse_manga_detail(data: &ResponseData) -> Manga {
	let cover = if data.manga_pic_image_url.is_empty() {
		data.manga_cover_image_url.clone()
	} else {
		data.manga_pic_image_url.clone()
	};
	let categories: Vec<String> = data
		.manga_theme
		.split(" ")
		.filter(|c| !c.is_empty())
		.map(|c| c.to_string())
		.collect();
	let status = match data.manga_is_over {
		0 => MangaStatus::Ongoing,
		1 => MangaStatus::Completed,
		_ => MangaStatus::Unknown,
	};
	Manga {
		key: data.manga_id.clone(),
		title: data.manga_name.clone(),
		cover: Some(cover),
		authors: Some(aidoku::alloc::vec![data.manga_author.trim().to_string()]),
		description: Some(data.manga_intro.clone()),
		url: Some(data.share_url.clone()),
		tags: Some(categories),
		status,
		content_rating: ContentRating::Safe,
		viewer: Viewer::RightToLeft,
		..Default::default()
	}
}

pub fn parse_chapter_list(data: &ResponseData) -> Vec<Chapter> {
	let mut chapters: Vec<Chapter> = Vec::new();
	chapters.append(&mut parse_chapters(&data.manga_words));
	chapters.append(&mut parse_chapters(&data.manga_rolls));
	chapters.append(&mut parse_chapters(&data.manga_episode));
	chapters.sort_by_key(|a| a.chapter_number.unwrap_or(0.0).to_bits());
	chapters.reverse();
	chapters
}

fn parse_chapters(items: &[ChapterItem]) -> Vec<Chapter> {
	items
		.iter()
		.map(|item| {
			let title = if item.section_title.is_empty() {
				item.section_name.clone()
			} else if item.is_must_pay == 0 {
				aidoku::alloc::format!("{} {}", item.section_name, item.section_title)
			} else {
				aidoku::alloc::format!("{} {} {}", "\u{1F512}", item.section_name, item.section_title)
			};
			let url = helper::gen_chapter_url(&item.section_id);
			Chapter {
				key: item.section_id.clone(),
				title: Some(title),
				chapter_number: Some(item.section_sort),
				url: Some(url),
				..Default::default()
			}
		})
		.collect()
}

pub fn parse_page_list(data: &PageData) -> Vec<Page> {
	if data.host_list.is_empty() {
		return Vec::new();
	}
	let host = &data.host_list[0];
	data.manga_section_images
		.iter()
		.map(|item| {
			let url = aidoku::alloc::format!("{}{}{}", host, item, data.query);
			Page {
				content: PageContent::url(url),
				..Default::default()
			}
		})
		.collect()
}
