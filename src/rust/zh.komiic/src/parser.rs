use aidoku::{
	alloc::{String, Vec},
	Chapter, ContentRating, Manga, MangaStatus, Page, PageContent, Viewer,
};
use serde::Deserialize;

use crate::helper;

#[derive(Deserialize)]
pub struct GqlResponse<T> {
	pub data: T,
}

#[derive(Deserialize)]
pub struct ComicByCategoriesData {
	#[serde(rename = "comicByCategories")]
	pub comic_by_categories: Vec<ComicItem>,
}

#[derive(Deserialize)]
pub struct RecentUpdateData {
	#[serde(rename = "recentUpdate")]
	pub recent_update: Vec<ComicItem>,
}

#[derive(Deserialize)]
pub struct HotComicsData {
	#[serde(rename = "hotComics")]
	pub hot_comics: Vec<ComicItem>,
}

#[derive(Deserialize)]
pub struct SearchData {
	#[serde(rename = "searchComicsAndAuthors")]
	pub search_comics_and_authors: SearchResult,
}

#[derive(Deserialize)]
pub struct SearchResult {
	pub comics: Vec<ComicItem>,
}

#[derive(Deserialize)]
pub struct ComicByIdData {
	#[serde(rename = "comicById")]
	pub comic_by_id: ComicItem,
}

#[derive(Deserialize)]
pub struct ChaptersByComicIdData {
	#[serde(rename = "chaptersByComicId")]
	pub chapters_by_comic_id: Vec<ChapterItem>,
}

#[derive(Deserialize)]
pub struct ImagesByChapterIdData {
	#[serde(rename = "imagesByChapterId")]
	pub images_by_chapter_id: Vec<ImageItem>,
}

#[derive(Deserialize)]
pub struct ComicItem {
	pub id: String,
	pub title: String,
	#[serde(default)]
	pub status: String,
	#[serde(rename = "imageUrl")]
	pub image_url: String,
	#[serde(default)]
	pub authors: Vec<AuthorItem>,
	#[serde(default)]
	pub categories: Vec<CategoryItem>,
}

#[derive(Deserialize)]
pub struct AuthorItem {
	pub name: String,
}

#[derive(Deserialize)]
pub struct CategoryItem {
	pub name: String,
}

#[derive(Deserialize)]
pub struct ChapterItem {
	pub id: String,
	pub serial: String,
	#[serde(rename = "type")]
	pub chapter_type: String,
}

#[derive(Deserialize)]
pub struct ImageItem {
	pub kid: String,
}

pub fn parse_manga_list(items: &[ComicItem]) -> Vec<Manga> {
	items.iter().map(parse_manga).collect()
}

pub fn parse_manga(item: &ComicItem) -> Manga {
	let authors: Vec<String> = item.authors.iter().map(|a| a.name.clone()).collect();
	let tags: Vec<String> = item.categories.iter().map(|c| c.name.clone()).collect();
	let status = match item.status.as_str() {
		"ONGOING" => MangaStatus::Ongoing,
		"END" => MangaStatus::Completed,
		_ => MangaStatus::Unknown,
	};
	Manga {
		key: item.id.clone(),
		title: item.title.clone(),
		cover: Some(item.image_url.clone()),
		authors: Some(authors),
		url: Some(helper::gen_manga_url(&item.id)),
		tags: Some(tags),
		status,
		content_rating: ContentRating::Safe,
		viewer: Viewer::RightToLeft,
		..Default::default()
	}
}

pub fn parse_chapter_list(manga_id: &str, items: &[ChapterItem]) -> Vec<Chapter> {
	let mut volumes: Vec<Chapter> = Vec::new();
	let mut chapters: Vec<Chapter> = Vec::new();

	for item in items {
		let url = helper::gen_chapter_url(manga_id, &item.id);
		if item.chapter_type == "book" {
			let volume_num = item.serial.parse::<f32>().unwrap_or(0.0);
			volumes.push(Chapter {
				key: item.id.clone(),
				title: Some(item.serial.clone()),
				volume_number: Some(volume_num),
				chapter_number: Some(-1.0),
				url: Some(url),
				..Default::default()
			});
		} else {
			let chapter_num = item.serial.parse::<f32>().unwrap_or(0.0);
			chapters.push(Chapter {
				key: item.id.clone(),
				title: Some(item.serial.clone()),
				volume_number: Some(-1.0),
				chapter_number: Some(chapter_num),
				url: Some(url),
				..Default::default()
			});
		}
	}

	let mut all_chapters = volumes;
	all_chapters.extend(chapters);
	all_chapters.reverse();
	all_chapters
}

pub fn parse_page_list(manga_id: &str, chapter_id: &str, items: &[ImageItem]) -> Vec<Page> {
	items
		.iter()
		.map(|item| {
			let url = helper::gen_page_url(manga_id, chapter_id, &item.kid);
			Page {
				content: PageContent::url(url),
				..Default::default()
			}
		})
		.collect()
}
