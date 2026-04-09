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

const WWW_URL: &str = "https://bakamh.com";
struct BakamhSource;

fn parse_manga_list(
	html: aidoku::imports::html::Document,
	search_mode: bool,
) -> Result<MangaPageResult> {
	let mut entries: Vec<Manga> = Vec::new();

	let selector = if search_mode {
		".c-tabs-item__content"
	} else {
		".page-item-detail"
	};

	if let Some(items) = html.select(selector) {
		for item in items {
			let (href_selector, img_selector, title_selector, suffix) = if search_mode {
				(
					".col-4>.tab-thumb>a",
					".col-4>.tab-thumb>a>img",
					".col-8>.tab-summary>.post-title>h3>a",
					"-193x278",
				)
			} else {
				(
					".item-thumb>a",
					".item-thumb>a>img",
					".item-summary>.post-title>h3>a",
					"-175x238",
				)
			};

			let key = item
				.select_first(href_selector)
				.and_then(|e| e.attr("href"))
				.unwrap_or_default()
				.split('/')
				.filter(|a| !a.is_empty())
				.map(|a| a.to_string())
				.collect::<Vec<String>>()
				.pop()
				.unwrap_or_default();
			let cover = item
				.select_first(img_selector)
				.and_then(|e| e.attr("src"))
				.unwrap_or_default()
				.replace(suffix, "");
			let title = item
				.select_first(title_selector)
				.and_then(|e| e.text())
				.unwrap_or_default();
			entries.push(Manga {
				key,
				cover: Some(cover),
				title,
				..Default::default()
			});
		}
	}

	let has_next_page = !entries.is_empty();
	Ok(MangaPageResult {
		entries,
		has_next_page,
	})
}

impl Source for BakamhSource {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut finish = String::new();

		for filter in filters {
			if let FilterValue::Select { id, value } = filter {
				if id == "finish" {
					finish = value;
				}
			}
		}

		let (url, search_mode) = if let Some(query) = query {
			(
				format!(
					"{}/page/{}/?s={}&post_type=wp-manga",
					WWW_URL,
					page,
					encode_uri(query)
				),
				true,
			)
		} else {
			(
				format!("{}/{}/page/{}/", WWW_URL, finish, page),
				false,
			)
		};

		let html = Request::get(&url)?.html()?;
		parse_manga_list(html, search_mode)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{}/manga/{}/", WWW_URL, manga.key);
		let html = if needs_details || needs_chapters {
			Some(Request::get(&url)?.html()?)
		} else {
			None
		};

		if needs_details {
			let html = html.as_ref().unwrap();
			manga.cover = html
				.select_first("meta[property='og:image']")
				.and_then(|e| e.attr("content"));
			manga.title = html
				.select_first("meta[property='og:title']")
				.and_then(|e| e.attr("content"))
				.unwrap_or_default();
			let authors = html
				.select(".author-content>a")
				.map(|items| items.filter_map(|a| a.text()).collect::<Vec<String>>())
				.unwrap_or_default();
			manga.authors = Some(authors);
			let len = html.select(".post-content>div").map(|items| items.count()).unwrap_or(0);
			manga.description = html
				.select_first(format!(".post-content>div:nth-child({})>div>p", len))
				.and_then(|e| e.text());
			manga.tags = Some(
				html.select(".tags-content>a")
					.map(|items| items.filter_map(|a| a.text()).collect::<Vec<String>>())
					.unwrap_or_default(),
			);
			manga.status = match html
				.select_first(format!(".post-content>div:nth-child({})>.summary-content", len.saturating_sub(2)))
				.and_then(|e| e.text())
				.unwrap_or_default()
				.trim()
			{
				"OnGoing" => MangaStatus::Ongoing,
				"Completed" => MangaStatus::Completed,
				_ => MangaStatus::Unknown,
			};
			manga.content_rating = ContentRating::NSFW;
			manga.viewer = Viewer::Webtoon;
			manga.url = Some(url.clone());
		}

		if needs_chapters {
			let html = html.as_ref().unwrap();
			let mut chapter_items: Vec<(String, String, String)> = Vec::new();
			if let Some(items) = html.select(".wp-manga-chapter>a") {
				for item in items {
					let chapter_url = item.attr("href").unwrap_or_default();
					let key = chapter_url
						.split('/')
						.filter(|a| !a.is_empty())
						.map(|a| a.to_string())
						.collect::<Vec<String>>()
						.pop()
						.unwrap_or_default();
					let title = item.text().unwrap_or_default().trim().to_string();
					chapter_items.push((key, title, chapter_url));
				}
			}
			let len = chapter_items.len();
			let mut chapters: Vec<Chapter> = Vec::new();
			for (index, (key, title, chapter_url)) in chapter_items.into_iter().enumerate() {
				chapters.push(Chapter {
					key,
					title: Some(title),
					chapter_number: Some((len.saturating_sub(index)) as f32),
					url: Some(chapter_url),
					..Default::default()
				});
			}
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{}/manga/{}/{}", WWW_URL, manga.key, chapter.key);
		let html = Request::get(&url)?.html()?;
		let mut pages: Vec<Page> = Vec::new();

		if let Some(items) = html.select("img[id]") {
			for item in items {
				let url = item.attr("src").unwrap_or_default().trim().to_string();
				pages.push(Page {
					content: PageContent::url(url),
					..Default::default()
				});
			}
		}

		Ok(pages)
	}
}

impl ListingProvider for BakamhSource {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let name = match listing.id.as_str() {
			"新作" => "newmanga",
			_ => return self.get_search_manga_list(None, page, Vec::new()),
		};
		let url = format!("{}/{}/page/{}/", WWW_URL, name, page);
		let html = Request::get(&url)?.html()?;
		parse_manga_list(html, false)
	}
}

impl ImageRequestProvider for BakamhSource {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		Ok(Request::get(&url)?.header("Referer", WWW_URL))
	}
}

register_source!(BakamhSource, ListingProvider, ImageRequestProvider);
