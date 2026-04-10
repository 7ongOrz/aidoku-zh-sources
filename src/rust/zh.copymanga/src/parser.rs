use core::str::FromStr;

use aidoku::{
	alloc::{String, Vec},
	prelude::*,
	Chapter, ContentRating, Manga, MangaStatus, Page, PageContent, Viewer,
};
use aidoku::alloc::string::ToString;
use serde::Deserialize;
use uuid::Uuid;

use crate::helper;

#[derive(Deserialize)]
pub struct ListResponse {
	pub results: ListResults,
}

#[derive(Deserialize)]
pub struct ListResults {
	#[serde(default)]
	pub total: i32,
	#[serde(default)]
	pub limit: i32,
	#[serde(default)]
	pub offset: i32,
	#[serde(default)]
	pub list: Vec<ListItem>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum ListItem {
	Wrapped { comic: MangaData },
	Direct(MangaData),
}

impl ListItem {
	fn into_data(self) -> MangaData {
		match self {
			ListItem::Wrapped { comic } => comic,
			ListItem::Direct(data) => data,
		}
	}
}

#[derive(Deserialize)]
pub struct MangaData {
	pub path_word: String,
	#[serde(default)]
	pub cover: String,
	#[serde(default)]
	pub name: String,
	#[serde(default)]
	pub author: Vec<NameItem>,
	#[serde(default)]
	pub brief: String,
	#[serde(default)]
	pub theme: Vec<NameItem>,
	#[serde(default)]
	pub status: Option<ValueItem>,
	#[serde(default)]
	pub restrict: Option<ValueItem>,
}

#[derive(Deserialize)]
pub struct NameItem {
	#[serde(default)]
	pub name: String,
}

#[derive(Deserialize)]
pub struct ValueItem {
	#[serde(default)]
	pub value: i32,
}

#[derive(Deserialize)]
pub struct EncryptedResponse {
	pub results: String,
}

#[derive(Deserialize, Default)]
pub struct ChapterRoot {
	#[serde(default)]
	pub build: BuildData,
	#[serde(default)]
	pub groups: Groups,
}

#[derive(Deserialize, Default)]
pub struct BuildData {
	#[serde(default)]
	pub path_word: String,
}

#[derive(Deserialize, Default)]
pub struct Groups {
	#[serde(default)]
	pub default: Option<Group>,
	#[serde(default)]
	pub tankobon: Option<Group>,
	#[serde(default)]
	pub other_honyakuchimu: Option<Group>,
	#[serde(default)]
	pub karapeji: Option<Group>,
}

#[derive(Deserialize, Default)]
pub struct Group {
	#[serde(default)]
	pub chapters: Vec<ChapterItem>,
}

#[derive(Deserialize)]
pub struct ChapterItem {
	pub id: String,
	#[serde(default)]
	pub name: String,
}

#[derive(Deserialize)]
pub struct PageItem {
	pub url: String,
}

pub fn parse_manga_list(list: Vec<ListItem>) -> Vec<Manga> {
	list.into_iter().map(|i| parse_manga(i.into_data())).collect()
}

pub fn parse_manga(data: MangaData) -> Manga {
	let authors: Vec<String> = data
		.author
		.into_iter()
		.map(|a| a.name)
		.filter(|n| !n.is_empty())
		.collect();
	let tags: Vec<String> = data
		.theme
		.into_iter()
		.map(|t| t.name)
		.filter(|n| !n.is_empty())
		.collect();
	let status = match data.status.as_ref().map(|s| s.value).unwrap_or(-1) {
		0 => MangaStatus::Ongoing,
		1 => MangaStatus::Completed,
		_ => MangaStatus::Unknown,
	};
	let content_rating = match data.restrict.as_ref().map(|r| r.value).unwrap_or(-1) {
		0 => ContentRating::Safe,
		1 => ContentRating::Suggestive,
		_ => ContentRating::NSFW,
	};
	let url = helper::gen_manga_url(&data.path_word);
	Manga {
		key: data.path_word,
		title: data.name,
		cover: Some(data.cover),
		authors: Some(authors),
		description: Some(data.brief.trim().to_string()),
		url: Some(url),
		tags: Some(tags),
		status,
		content_rating,
		viewer: Viewer::RightToLeft,
		..Default::default()
	}
}

fn parse_chapter_group(
	manga_id: &str,
	group: Option<Group>,
	name: &str,
	start: usize,
) -> Vec<Chapter> {
	let list = match group {
		Some(g) => g.chapters,
		None => return Vec::new(),
	};
	list.into_iter()
		.enumerate()
		.map(|(index, item)| {
			let chapter_number = (index + start + 1) as f32;
			let date_uploaded = Uuid::from_str(&item.id)
				.ok()
				.and_then(|u| u.get_timestamp())
				.map(|ts| ts.to_unix().0 as i64);
			let url = helper::gen_chapter_url(manga_id, &item.id);
			Chapter {
				key: item.id,
				title: Some(format!("{} - {}", name, item.name)),
				chapter_number: Some(chapter_number),
				date_uploaded,
				url: Some(url),
				..Default::default()
			}
		})
		.collect()
}

pub fn parse_chapter_list(data: ChapterRoot) -> Vec<Chapter> {
	let manga_id = data.build.path_word;
	let default = parse_chapter_group(&manga_id, data.groups.default, "默认", 0);
	let tankobon = parse_chapter_group(
		&manga_id,
		data.groups.tankobon,
		"单行本",
		default.len(),
	);
	let other_honyakuchimu = parse_chapter_group(
		&manga_id,
		data.groups.other_honyakuchimu,
		"其它汉化版",
		default.len() + tankobon.len(),
	);
	let karapeji = parse_chapter_group(
		&manga_id,
		data.groups.karapeji,
		"全彩版",
		default.len() + tankobon.len() + other_honyakuchimu.len(),
	);
	let mut chapters = [default, tankobon, other_honyakuchimu, karapeji].concat();
	chapters.reverse();
	chapters
}

pub fn parse_page_list(pages: Vec<PageItem>) -> Vec<Page> {
	pages
		.into_iter()
		.map(|p| Page {
			content: PageContent::url(p.url),
			..Default::default()
		})
		.collect()
}
