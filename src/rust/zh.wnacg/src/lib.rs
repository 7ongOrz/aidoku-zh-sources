#![no_std]
use aidoku::{
	alloc::{String, Vec},
	helpers::uri::encode_uri,
	imports::net::Request,
	prelude::*,
	Chapter, ContentRating, FilterValue, ImageRequestProvider, Listing, ListingProvider, Manga,
	MangaPageResult, MangaStatus, Page, PageContent, Result, Source, Viewer,
};
use aidoku::alloc::string::ToString;

const WWW_URL: &str = "https://www.wnacg.ru";
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36";

const FILTER_CATEGORY_5: [&str; 4] = ["5", "1", "12", "16"];
const FILTER_CATEGORY_6: [&str; 4] = ["6", "9", "13", "17"];
const FILTER_CATEGORY_7: [&str; 4] = ["7", "10", "14", "18"];

struct WnacgSource;

fn parse_manga_list(html: aidoku::imports::html::Document) -> Result<MangaPageResult> {
	let mut entries: Vec<Manga> = Vec::new();

	if let Some(items) = html.select(".gallary_item") {
		for item in items {
			let key = item
				.select_first(".pic_box>a")
				.and_then(|e| e.attr("href"))
				.unwrap_or_default()
				.split("-")
				.map(|a| a.replace(".html", ""))
				.collect::<Vec<String>>()
				.pop()
				.unwrap_or_default();
			let cover = format!(
				"https:{}",
				item.select_first(".pic_box>a>img")
					.and_then(|e| e.attr("src"))
					.unwrap_or_default()
			);
			let title = item
				.select_first(".info>.title>a")
				.and_then(|e| e.text())
				.unwrap_or_default()
				.trim()
				.to_string();
			entries.push(Manga {
				key,
				title,
				cover: Some(cover),
				..Default::default()
			});
		}
	}

	Ok(MangaPageResult {
		has_next_page: !entries.is_empty(),
		entries,
	})
}

impl Source for WnacgSource {
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

		for filter in filters {
			if let FilterValue::Select { id, value } = filter {
				match id.as_str() {
					"category" => {
						category = value;
					}
					"language" => {
						let index = value.parse::<usize>().unwrap_or(0);
						category = match category.as_str() {
							"5" => FILTER_CATEGORY_5.get(index).unwrap_or(&"5").to_string(),
							"6" => FILTER_CATEGORY_6.get(index).unwrap_or(&"6").to_string(),
							"7" => FILTER_CATEGORY_7.get(index).unwrap_or(&"7").to_string(),
							_ => category,
						};
					}
					_ => {}
				}
			}
		}

		let url = if let Some(query) = query {
			format!(
				"{}/search/index.php?q={}&s=create_time_DESC&syn=yes&p={}",
				WWW_URL,
				encode_uri(query),
				page
			)
		} else {
			format!(
				"{}/albums-index-page-{}-cate-{}.html",
				WWW_URL, page, category
			)
		};

		let html = Request::get(&url)?.header("User-Agent", UA).html()?;
		parse_manga_list(html)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			let url = format!("{}/photos-index-aid-{}.html", WWW_URL, manga.key);
			let html = Request::get(&url)?.header("User-Agent", UA).html()?;

			let cover = html
				.select_first("#bodywrap>div>.uwthumb>img")
				.and_then(|e| e.attr("src"))
				.unwrap_or_default()
				.replace("//", "");
			manga.cover = Some(format!("https://{}", cover));
			manga.title = html
				.select_first("#bodywrap>h2")
				.and_then(|e| e.text())
				.unwrap_or_default();

			let categories_text = html
				.select_first("#bodywrap>div>.uwconn>label:nth-child(1)")
				.and_then(|e| e.text())
				.unwrap_or_default()
				.replace("分類：", "");
			let categories: Vec<String> = categories_text
				.split("／")
				.flat_map(|a| a.split("&"))
				.map(|a| a.trim().to_string())
				.collect();

			let mut tags = categories;
			if let Some(tag_elements) = html.select("#bodywrap>div>.uwconn>.addtags>.tagshow") {
				for tag in tag_elements {
					if let Some(t) = tag.text() {
						tags.push(t.trim().to_string());
					}
				}
			}
			manga.tags = Some(tags);
			manga.status = MangaStatus::Unknown;
			manga.content_rating = ContentRating::NSFW;
			manga.viewer = Viewer::RightToLeft;
			manga.url = Some(url);
		}

		if needs_chapters {
			manga.chapters = Some(aidoku::alloc::vec![Chapter {
				key: manga.key.clone(),
				title: Some(String::from("第 1 话")),
				chapter_number: Some(1.0),
				url: Some(format!(
					"{}/photos-index-aid-{}.html",
					WWW_URL, manga.key
				)),
				..Default::default()
			}]);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, _chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{}/photos-gallery-aid-{}.html", WWW_URL, manga.key);
		let text = Request::get(&url)?.header("User-Agent", UA).string()?;
		let pages: Vec<Page> = text
			.split("\\\"")
			.filter(|a| a.starts_with("//"))
			.map(|item| Page {
				content: PageContent::url(format!("https:{}", item)),
				..Default::default()
			})
			.collect();
		Ok(pages)
	}
}

impl ListingProvider for WnacgSource {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let category = match listing.id.as_str() {
			"CG画集" => "2",
			"3D漫画" => "22",
			"Cosplay" => "3",
			"韩漫" => "19",
			_ => return self.get_search_manga_list(None, page, Vec::new()),
		};
		let url = format!(
			"{}/albums-index-page-{}-cate-{}.html",
			WWW_URL, page, category
		);
		let html = Request::get(&url)?.header("User-Agent", UA).html()?;
		parse_manga_list(html)
	}
}

impl ImageRequestProvider for WnacgSource {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		Ok(Request::get(&url)?.header("Referer", WWW_URL))
	}
}

register_source!(WnacgSource, ListingProvider, ImageRequestProvider);
