use core::str::FromStr;

use aidoku::{
	alloc::{String, Vec},
	alloc::collections::BTreeMap,
	alloc::string::ToString,
	prelude::*,
	Chapter, ContentRating, Manga, MangaStatus, Page, PageContent, Viewer,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::helper;

// ---------- 公开列表接口 (/comics /ranks /recs /update/newest /search/comic) ----------

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

// ---------- /comic2/{path_word} ----------

#[derive(Deserialize)]
pub struct GetComicResults {
	pub comic: ComicDetail,
	#[serde(default)]
	pub groups: BTreeMap<String, GroupData>,
}

#[derive(Deserialize)]
pub struct ComicDetail {
	#[serde(default)]
	pub name: String,
	#[serde(default)]
	pub path_word: String,
	#[serde(default)]
	pub cover: String,
	#[serde(default)]
	pub brief: String,
	#[serde(default)]
	pub author: Vec<NameItem>,
	#[serde(default)]
	pub theme: Vec<NameItem>,
	#[serde(default)]
	pub status: Option<ValueItem>,
	#[serde(default)]
	pub restrict: Option<ValueItem>,
}

#[derive(Deserialize, Clone)]
pub struct GroupData {
	pub path_word: String,
	#[serde(default)]
	pub name: String,
}

// ---------- /comic/{path}/group/{g}/chapters ----------

#[derive(Deserialize)]
pub struct GetChaptersResults {
	#[serde(default)]
	pub total: i32,
	#[serde(default)]
	pub list: Vec<ChapterItem>,
}

#[derive(Deserialize)]
pub struct ChapterItem {
	pub uuid: String,
	#[serde(default)]
	pub name: String,
}

// ---------- /comic/{path}/chapter2/{uuid} ----------

#[derive(Deserialize)]
pub struct GetChapterResults {
	pub chapter: ChapterDetail,
}

#[derive(Deserialize)]
pub struct ChapterDetail {
	#[serde(default)]
	pub contents: Vec<ContentItem>,
	#[serde(default)]
	pub words: Vec<i32>,
}

#[derive(Deserialize)]
pub struct ContentItem {
	pub url: String,
}

// ---------- 转换函数 ----------

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
	let url = helper::gen_web_manga_url(&data.path_word);
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

pub fn fill_manga_details(manga: &mut Manga, detail: ComicDetail) {
	if !detail.cover.is_empty() {
		manga.cover = Some(detail.cover);
	}
	if !detail.name.is_empty() {
		manga.title = detail.name;
	}
	let authors: Vec<String> = detail
		.author
		.into_iter()
		.map(|a| a.name)
		.filter(|n| !n.is_empty())
		.collect();
	if !authors.is_empty() {
		manga.authors = Some(authors);
	}
	let tags: Vec<String> = detail
		.theme
		.into_iter()
		.map(|t| t.name)
		.filter(|n| !n.is_empty())
		.collect();
	if !tags.is_empty() {
		manga.tags = Some(tags);
	}
	if !detail.brief.is_empty() {
		manga.description = Some(detail.brief.trim().to_string());
	}
	manga.status = match detail.status.as_ref().map(|s| s.value).unwrap_or(-1) {
		0 => MangaStatus::Ongoing,
		1 => MangaStatus::Completed,
		_ => MangaStatus::Unknown,
	};
	manga.content_rating = match detail.restrict.as_ref().map(|r| r.value).unwrap_or(-1) {
		0 => ContentRating::Safe,
		1 => ContentRating::Suggestive,
		_ => ContentRating::NSFW,
	};
	manga.viewer = Viewer::RightToLeft;
	manga.url = Some(helper::gen_web_manga_url(&detail.path_word));
}

/// 按官方优先顺序排列 groups：default → tankobon → other_honyakuchimu → karapeji → 其余按字典序
pub fn ordered_groups(groups: BTreeMap<String, GroupData>) -> Vec<GroupData> {
	const PRIORITY: &[&str] = &["default", "tankobon", "other_honyakuchimu", "karapeji"];
	let mut ordered: Vec<GroupData> = Vec::new();
	let mut remaining = groups;
	for key in PRIORITY {
		if let Some(g) = remaining.remove(*key) {
			ordered.push(g);
		}
	}
	for (_, g) in remaining {
		ordered.push(g);
	}
	ordered
}

pub fn parse_chapters(
	manga_path: &str,
	group_name: &str,
	list: Vec<ChapterItem>,
	start_index: usize,
) -> Vec<Chapter> {
	list.into_iter()
		.enumerate()
		.map(|(idx, item)| {
			let chapter_number = (start_index + idx + 1) as f32;
			let date_uploaded = Uuid::from_str(&item.uuid)
				.ok()
				.and_then(|u| u.get_timestamp())
				.map(|ts| ts.to_unix().0 as i64);
			let url = helper::gen_web_chapter_url(manga_path, &item.uuid);
			Chapter {
				key: item.uuid,
				title: Some(format!("{} - {}", group_name, item.name)),
				chapter_number: Some(chapter_number),
				date_uploaded,
				url: Some(url),
				..Default::default()
			}
		})
		.collect()
}

/// 按 words 索引数组重排 contents。若长度不匹配则回退原顺序。
pub fn parse_page_list(detail: ChapterDetail) -> Vec<Page> {
	let ChapterDetail { contents, words } = detail;
	let urls: Vec<String> = if !words.is_empty() && words.len() == contents.len() {
		let mut pairs: Vec<(i32, String)> = contents
			.into_iter()
			.zip(words.into_iter())
			.map(|(c, w)| (w, c.url))
			.collect();
		pairs.sort_by_key(|(w, _)| *w);
		pairs.into_iter().map(|(_, u)| u).collect()
	} else {
		contents.into_iter().map(|c| c.url).collect()
	};
	urls.into_iter()
		.map(|url| Page {
			content: PageContent::url(url),
			..Default::default()
		})
		.collect()
}
