#![no_std]
use aidoku::{
	alloc::{String, Vec},
	helpers::uri::encode_uri,
	imports::net::Request,
	prelude::*,
	Chapter, ContentRating, FilterValue, Manga, MangaPageResult, MangaStatus, Page, PageContent,
	Result, Source, Viewer,
};
use aidoku::alloc::string::ToString;
use serde::Deserialize;

const WWW_URL: &str = "https://www.baozimh.com";
const IMG_URL: &str = "https://static-tw.baozimh.com";

const FILTER_CATEGORY: [&str; 26] = [
	"all",
	"lianai",
	"chunai",
	"gufeng",
	"yineng",
	"xuanyi",
	"juqing",
	"kehuan",
	"qihuan",
	"xuanhuan",
	"chuanyue",
	"maoxian",
	"tuili",
	"wuxia",
	"gedou",
	"zhanzheng",
	"rexie",
	"gaoxiao",
	"danuzhu",
	"dushi",
	"zongcai",
	"hougong",
	"richang",
	"hanman",
	"shaonian",
	"qita",
];
const FILTER_REGION: [&str; 5] = ["all", "cn", "jp", "kr", "en"];
const FILTER_STATUS: [&str; 3] = ["all", "serial", "pub"];

#[derive(Deserialize)]
struct ListResponse {
	items: Vec<ListItem>,
}

#[derive(Deserialize)]
struct ListItem {
	comic_id: String,
	topic_img: String,
	name: String,
}

struct BaozimhSource;

impl Source for BaozimhSource {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut category = String::new();
		let mut region = String::new();
		let mut status = String::new();

		for filter in filters {
			if let FilterValue::Select { id, value } = filter {
				let index = value.parse::<usize>().unwrap_or(0);
				match id.as_str() {
					"category" => {
						category = FILTER_CATEGORY[index].to_string();
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
		}

		let mut entries: Vec<Manga> = Vec::new();

		if let Some(query) = query {
			let url = format!("{}/search/?q={}", WWW_URL, encode_uri(query));
			let html = Request::get(&url)?.html()?;

			if let Some(items) = html.select(".pure-g>.comics-card") {
				for item in items {
					let key = item
						.select_first(".comics-card__info")
						.and_then(|e| e.attr("href"))
						.unwrap_or_default()
						.split("/")
						.filter(|a| !a.is_empty())
						.map(|a| a.to_string())
						.collect::<Vec<String>>()
						.pop()
						.unwrap_or_default();
					let cover = item
						.select_first(".comics-card__poster>amp-img")
						.and_then(|e| e.attr("src"));
					let title = item
						.select_first(".comics-card__title")
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
			let url = format!(
				"{}/api/bzmhq/amp_comic_list?type={}&region={}&state={}&page={}&language=tw",
				WWW_URL, category, region, status, page
			);
			let resp: ListResponse = Request::get(&url)?.json_owned()?;

			for item in resp.items {
				let cover = format!(
					"{}/cover/{}?w=285&h=375&q=100",
					IMG_URL, item.topic_img
				);
				entries.push(Manga {
					key: item.comic_id,
					cover: Some(cover),
					title: item.name,
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
			let url = format!("{}/comic/{}", WWW_URL, manga.key);
			let html = Request::get(&url)?.html()?;

			manga.key = html
				.select_first("meta[name='og:novel:read_url']")
				.and_then(|e| e.attr("content"))
				.unwrap_or_default()
				.split("/")
				.filter(|a| !a.is_empty())
				.map(|a| a.to_string())
				.collect::<Vec<String>>()
				.pop()
				.unwrap_or_default();
			manga.cover = html
				.select_first("meta[name='og:image'")
				.and_then(|e| e.attr("content"));
			manga.title = html
				.select_first("meta[name='og:novel:book_name']")
				.and_then(|e| e.attr("content"))
				.unwrap_or_default();
			manga.authors = html
				.select_first("meta[name='og:novel:author']")
				.and_then(|e| e.attr("content"))
				.map(|a| aidoku::alloc::vec![a]);
			manga.description = html
				.select_first("meta[name='og:description']")
				.and_then(|e| e.attr("content"))
				.map(|s| {
					s.split(",")
						.skip(2)
						.map(|a| a.to_string())
						.collect::<Vec<String>>()
						.join(", ")
				});
			let tags: Vec<String> = html
				.select_first("meta[name='og:novel:category']")
				.and_then(|e| e.attr("content"))
				.unwrap_or_default()
				.split(",")
				.map(|a| a.to_string())
				.filter(|a| !a.starts_with("types"))
				.collect();
			manga.tags = Some(tags);

			let status_text = html
				.select_first("meta[name='og:novel:status']")
				.and_then(|e| e.attr("content"))
				.unwrap_or_default();
			manga.status = match status_text.as_str() {
				"連載中" => MangaStatus::Ongoing,
				"已完結" => MangaStatus::Completed,
				_ => MangaStatus::Unknown,
			};
			manga.content_rating = ContentRating::Safe;
			manga.viewer = Viewer::RightToLeft;
			manga.url = Some(url);
		}

		if needs_chapters {
			let url = format!("{}/comic/{}", WWW_URL, manga.key);
			let html = Request::get(&url)?.html()?;
			let mut chapters: Vec<Chapter> = Vec::new();

			if let Some(items) = html.select("div[id^='chapter']>div>a") {
				for (index, item) in items.enumerate() {
					let chapter_id = item
						.attr("href")
						.unwrap_or_default()
						.split("&")
						.skip(1)
						.map(|a| a.split("=").nth(1).unwrap_or_default().to_string())
						.collect::<Vec<String>>()
						.join("_");
					let title = item
						.select_first("div>span")
						.and_then(|e| e.text());
					let chapter_url = format!(
						"{}/comic/chapter/{}/{}.html",
						WWW_URL, manga.key, chapter_id
					);
					chapters.push(Chapter {
						key: chapter_id,
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
		let mut pages: Vec<Page> = Vec::new();
		let mut current_chapter_id = chapter.key.clone();

		loop {
			let url = format!(
				"{}/comic/chapter/{}/{}.html",
				WWW_URL, manga.key, current_chapter_id
			);
			let html = Request::get(&url)?.html()?;

			let next_chapter_id = html
				.select_first("#next-chapter")
				.and_then(|e| e.attr("href"))
				.unwrap_or_default()
				.split("/")
				.map(|a| a.to_string())
				.collect::<Vec<String>>()
				.pop()
				.unwrap_or_default()
				.replace(".html", "");

			if let Some(items) = html.select("amp-img[id^='chapter-img']") {
				for item in items {
					let img_url = item
						.attr("src")
						.unwrap_or_default()
						.replace("fcomic", "scomic");
					pages.push(Page {
						content: PageContent::url(img_url),
						..Default::default()
					});
				}
			}

			if !next_chapter_id
				.starts_with(&current_chapter_id.split('_').collect::<Vec<&str>>()[..2].join("_"))
			{
				break;
			}

			current_chapter_id = next_chapter_id;
		}

		Ok(pages)
	}
}

register_source!(BaozimhSource);
