#![no_std]
use aidoku::{
	alloc::{String, Vec},
	helpers::uri::encode_uri,
	imports::net::Request,
	prelude::*,
	Chapter, ContentRating, FilterValue, Listing, ListingProvider, Manga, MangaPageResult,
	MangaStatus, Page, PageContent, Result, Source, Viewer,
};
use aidoku::alloc::string::ToString;

const WWW_URL: &str = "https://www.gufengmh.com";
const IMG_URL: &str = "https://res1.xiaoqinre.com";

const FILTER_GENRE: [&str; 5] = ["", "shaonian", "shaonv", "qingnian", "zhenrenmanhua"];
const FILTER_REGION: [&str; 6] = [
	"",
	"ribenmanhua",
	"guochanmanhua",
	"gangtaimanhua",
	"oumeimanhua",
	"hanguomanhua",
];
const FILTER_STATUS: [&str; 3] = ["", "wanjie", "lianzai"];
const FILTER_SORT: [&str; 3] = ["post", "update", "click"];

struct GufengmhSource;

impl Source for GufengmhSource {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut genre = String::new();
		let mut region = String::new();
		let mut status = String::new();
		let mut sort = String::from("click");

		for filter in filters {
			match filter {
				FilterValue::Select { id, value } => {
					let index = value.parse::<usize>().unwrap_or(0);
					match id.as_str() {
						"genre" => {
							genre = FILTER_GENRE[index].to_string();
						}
						"region" => {
							region = FILTER_REGION[index].to_string();
						}
						"status" => {
							status = FILTER_STATUS[index].to_string();
						}
						_ => {}
					}
				}
				FilterValue::Sort { id, index, ascending } => {
					if id == "sort" {
						if let Some(s) = FILTER_SORT.get(index as usize) {
							sort = s.to_string();
						}
						if ascending {
							sort = format!("-{}", sort);
						}
					}
				}
				_ => {}
			}
		}

		let url = if let Some(query) = query {
			format!(
				"{}/search/?keywords={}&page={}",
				WWW_URL,
				encode_uri(query),
				page
			)
		} else {
			format!(
				"{}/list/{}-{}-{}/{}/{}/",
				WWW_URL, genre, region, status, sort, page
			)
		};
		let html = Request::get(&url)?.html()?;
		let mut entries: Vec<Manga> = Vec::new();

		if let Some(items) = html.select(".book-list>li") {
			for item in items {
				let key = item
					.select_first(".cover")
					.and_then(|e| e.attr("href"))
					.unwrap_or_default()
					.split("/")
					.filter(|a| !a.is_empty())
					.map(|a| a.to_string())
					.collect::<Vec<String>>()
					.pop()
					.unwrap_or_default();
				let cover = item
					.select_first(".cover>img")
					.and_then(|e| e.attr("src"));
				let title = item
					.select_first(".ell>a")
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
			let url = format!("{}/manhua/{}/", WWW_URL, manga.key);
			let html = Request::get(&url)?.html()?;

			manga.cover = html
				.select_first(".book-cover>.cover>img")
				.and_then(|e| e.attr("src"));
			manga.title = html
				.select_first(".book-title>h1>span")
				.and_then(|e| e.text())
				.unwrap_or_default();
			let author_text = html
				.select_first("a[href*='author']")
				.and_then(|e| e.text())
				.unwrap_or_default();
			manga.authors = Some(aidoku::alloc::vec![author_text]);
			manga.description = html
				.select_first("#intro-cut>p")
				.and_then(|e| e.text())
				.map(|s| {
					s.replace("漫画简介：", "")
						.replace("介绍:", "")
						.trim()
						.to_string()
				});

			let mut tags = Vec::new();
			if let Some(items) = html.select(".detail-list>li:nth-child(2)>span:nth-child(1)>a") {
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
				.select_first(".detail-list>li:nth-child(1)>span:nth-child(1)>a")
				.and_then(|e| e.text())
				.unwrap_or_default();
			manga.status = match status_text.as_str() {
				"已完结" => MangaStatus::Completed,
				"连载中" => MangaStatus::Ongoing,
				_ => MangaStatus::Unknown,
			};
			manga.content_rating = ContentRating::Safe;
			manga.viewer = Viewer::Webtoon;
			manga.url = Some(url);
		}

		if needs_chapters {
			let url = format!("{}/manhua/{}/", WWW_URL, manga.key);
			let html = Request::get(&url)?.html()?;
			let mut chapters: Vec<Chapter> = Vec::new();

			if let Some(items) = html.select("#chapter-list-1>li>a") {
				for (index, item) in items.enumerate() {
					let key = item
						.attr("href")
						.unwrap_or_default()
						.split("/")
						.map(|a| a.to_string())
						.collect::<Vec<String>>()
						.pop()
						.unwrap_or_default()
						.replace(".html", "");
					let title = item
						.select_first("span")
						.and_then(|e| e.text());
					let chapter_url = format!(
						"{}/manhua/{}/{}.html",
						WWW_URL, manga.key, key
					);
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

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!(
			"{}/manhua/{}/{}.html",
			WWW_URL, manga.key, chapter.key
		);
		let html = Request::get(&url)?.html()?;
		let text = html
			.select_first("html")
			.and_then(|e| e.html())
			.unwrap_or_default();

		let list_str = text
			.split_once("var chapterImages = ")
			.and_then(|(_, after)| after.split_once(";"))
			.map(|(before, _)| before)
			.unwrap_or("[]");
		let path = text
			.split_once("var chapterPath = ")
			.and_then(|(_, after)| after.split_once(";"))
			.map(|(before, _)| before.replace("\"", ""))
			.unwrap_or_default();

		let image_list: Vec<String> = serde_json::from_str(list_str).unwrap_or_default();
		let pages: Vec<Page> = image_list
			.into_iter()
			.map(|item| Page {
				content: PageContent::url(format!("{}/{}{}", IMG_URL, path, item)),
				..Default::default()
			})
			.collect();

		Ok(pages)
	}
}

impl ListingProvider for GufengmhSource {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let name = match listing.id.as_str() {
			"总人气榜" => "popularity",
			"日人气榜" => "popularity-daily",
			"周人气榜" => "popularity-weekly",
			"月人气榜" => "popularity-monthly",
			"总点击榜" => "click",
			"日点击榜" => "click-daily",
			"周点击榜" => "click-weekly",
			"月点击榜" => "click-monthly",
			"总订阅榜" => "subscribe",
			_ => return self.get_search_manga_list(None, page, Vec::new()),
		};

		let url = format!("{}/rank/{}/", WWW_URL, name);
		let html = Request::get(&url)?.html()?;
		let mut entries: Vec<Manga> = Vec::new();

		if let Some(items) = html.select(".rank-list>li") {
			for item in items {
				let key = item
					.select_first(".cover")
					.and_then(|e| e.attr("href"))
					.unwrap_or_default()
					.split("/")
					.filter(|a| !a.is_empty())
					.map(|a| a.to_string())
					.collect::<Vec<String>>()
					.pop()
					.unwrap_or_default();
				let cover = item
					.select_first(".cover>img")
					.and_then(|e| e.attr("src"));
				let title = item
					.select_first(".ell>a")
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

		Ok(MangaPageResult {
			has_next_page: false,
			entries,
		})
	}
}

register_source!(GufengmhSource, ListingProvider);
