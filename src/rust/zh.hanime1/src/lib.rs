#![no_std]
use aidoku::{
	alloc::{vec, String, Vec},
	helpers::uri::encode_uri,
	imports::net::Request,
	prelude::*,
	Chapter, ContentRating, FilterValue, Listing, ListingProvider, Manga, MangaPageResult,
	MangaStatus, Page, PageContent, Result, Source, Viewer,
};
use aidoku::alloc::string::ToString;

const WWW_URL: &str = "https://hanime1.me";

struct Hanime1Source;

fn parse_manga_list(html: aidoku::imports::html::Document) -> Result<MangaPageResult> {
	let mut entries: Vec<Manga> = Vec::new();

	if let Some(items) = html.select(".comic-rows-videos-div>a") {
		for item in items {
			let key = item
				.attr("href")
				.unwrap_or_default()
				.split('/')
				.map(|a| a.to_string())
				.collect::<Vec<String>>()
				.pop()
				.unwrap_or_default();
			let cover = item.select_first("img").and_then(|e| e.attr("data-srcset"));
			let title = item
				.select_first("div>.comic-rows-videos-title")
				.and_then(|e| e.text())
				.unwrap_or_default();
			entries.push(Manga {
				key,
				cover,
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

impl Source for Hanime1Source {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		_filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let query = query.unwrap_or_default();
		let url = format!(
			"{}/comics/search?query={}&page={}",
			WWW_URL,
			encode_uri(query),
			page
		);
		let html = Request::get(&url)?.html()?;
		parse_manga_list(html)
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
			manga.cover = html
				.select_first("meta[property='og:image']")
				.and_then(|e| e.attr("content"));
			manga.title = html
				.select("h3[class^=title]>span")
				.map(|items| {
					items
						.filter_map(|a| a.text().map(|t| t.trim().to_string()))
						.collect::<Vec<String>>()
						.join(" ")
				})
				.unwrap_or_default();
			manga.authors = Some(vec![
				html
					.select_first("a[href*=artists]>div[style]")
					.and_then(|e| e.text())
					.unwrap_or_default(),
			]);
			manga.description = html
				.select_first("meta[property='og:description']")
				.and_then(|e| e.attr("content"))
				.map(|s| s.trim().to_string());
			manga.tags = Some(
				html.select("a[href*=tags]>div[style]")
					.map(|items| {
						items
							.filter_map(|a| a.text().map(|t| t.trim().to_string()))
							.collect::<Vec<String>>()
					})
					.unwrap_or_default(),
			);
			manga.status = MangaStatus::Unknown;
			manga.content_rating = ContentRating::NSFW;
			manga.viewer = Viewer::RightToLeft;
			manga.url = Some(url);
		}

		if needs_chapters {
			manga.chapters = Some(vec![Chapter {
				key: manga.key.clone(),
				title: Some(String::from("第 1 话")),
				chapter_number: Some(1.0),
				url: Some(format!("{}/comic/{}/1", WWW_URL, manga.key)),
				..Default::default()
			}]);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, _chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{}/comic/{}", WWW_URL, manga.key);
		let html = Request::get(&url)?.html()?;
		let mut pages: Vec<Page> = Vec::new();
		if let Some(items) = html.select(".comics-panel-margin>a>img") {
			for item in items {
				let url = item
					.attr("data-srcset")
					.unwrap_or_default()
					.replace("//t", "//i")
					.replace("t.", ".");
				pages.push(Page {
					content: PageContent::url(url),
					..Default::default()
				});
			}
		}
		Ok(pages)
	}
}

impl ListingProvider for Hanime1Source {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let sort = match listing.id.as_str() {
			"日榜" => "popular-today",
			"周榜" => "popular-week",
			"总榜" => "popular",
			_ => return self.get_search_manga_list(None, page, Vec::new()),
		};
		let url = format!(
			"{}/comics/search?sort={}&query=&page={}",
			WWW_URL, sort, page
		);
		let html = Request::get(&url)?.html()?;
		parse_manga_list(html)
	}
}

register_source!(Hanime1Source, ListingProvider);
