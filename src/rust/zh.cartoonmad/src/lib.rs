#![no_std]
use aidoku::{
	alloc::{String, Vec},
	helpers::uri::encode_uri,
	imports::{
		html::Html,
		net::Request,
	},
	prelude::*,
	Chapter, ContentRating, FilterValue, ImageRequestProvider, Listing, ListingProvider, Manga,
	MangaPageResult, MangaStatus, Page, PageContent, Result, Source, Viewer,
};
use aidoku::alloc::string::ToString;
use encoding_rs::BIG5;

const WWW_URL: &str = "https://www.cartoonmad.com";
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36";

fn handle_img_url(url: String) -> String {
	if url.starts_with("http") {
		return url;
	}
	format!("https:{}", url)
}

fn parse_big5_html(url: &str) -> Result<aidoku::imports::html::Document> {
	let data = Request::get(url)?.header("User-Agent", UA).data()?;
	let (decoded, _, _) = BIG5.decode(&data);
	Ok(Html::parse(decoded.as_bytes())?)
}

fn parse_manga_list(html: aidoku::imports::html::Document) -> Result<MangaPageResult> {
	let mut entries: Vec<Manga> = Vec::new();

	if let Some(items) = html.select(".comic_prev") {
		for item in items {
			let key = item
				.select_first(".a1")
				.and_then(|e| e.attr("href"))
				.unwrap_or_default()
				.split("/")
				.map(|a| a.to_string())
				.filter(|a| !a.is_empty())
				.collect::<Vec<String>>()
				.pop()
				.unwrap_or_default()
				.replace(".html", "");
			let cover = format!(
				"{}{}",
				WWW_URL,
				item.select_first("img")
					.and_then(|e| e.attr("src"))
					.unwrap_or_default()
			);
			let title = item
				.select_first(".covertxt+a")
				.and_then(|e| e.attr("title"))
				.unwrap_or_default();
			entries.push(Manga {
				key,
				cover: Some(cover),
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

struct CartoonmadSource;

impl Source for CartoonmadSource {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		_filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let url = if let Some(query) = query {
			format!("{}/m/?keyword={}", WWW_URL, encode_uri(query))
		} else {
			format!("{}/m/?page={}", WWW_URL, page)
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
		let url = format!("{}/m/comic/{}.html", WWW_URL, manga.key);

		if needs_details {
			let html = parse_big5_html(&url)?;
			let cover = format!(
				"{}{}",
				WWW_URL,
				html.select_first("link[rel='image_src']")
					.and_then(|e| e.attr("href"))
					.unwrap_or_default()
			);
			manga.cover = Some(cover);
			manga.title = html
				.select_first("meta[name='keywords']")
				.and_then(|e| e.attr("content"))
				.unwrap_or_default()
				.split(",")
				.map(|a| a.trim().to_string())
				.filter(|a| !a.is_empty())
				.collect::<Vec<String>>()
				.first()
				.unwrap_or(&String::new())
				.to_string();

			let mut authors = Vec::new();
			if let Some(items) = html.select("td[height='24']") {
				let items_vec: Vec<_> = items.collect();
				if let Some(item) = items_vec.get(1) {
					if let Some(text) = item.text() {
						let author = text.trim().replace("作者：", "");
						if !author.is_empty() {
							authors.push(author);
						}
					}
				}
			}
			manga.authors = Some(authors);

			let mut description = String::new();
			if let Some(items) = html.select("td[style='font-size:11pt;']") {
				let items_vec: Vec<_> = items.collect();
				if let Some(item) = items_vec.get(2) {
					description = item.text().unwrap_or_default();
				}
			}
			manga.description = Some(description);

			let mut tags = Vec::new();
			if let Some(items) = html.select("a[href*='tkey']") {
				for item in items {
					if let Some(t) = item.text() {
						tags.push(t);
					}
				}
			}
			manga.tags = Some(tags);
			manga.status = MangaStatus::Unknown;
			manga.content_rating = ContentRating::Safe;
			manga.viewer = Viewer::RightToLeft;
			manga.url = Some(url.clone());
		}

		if needs_chapters {
			let html = parse_big5_html(&url)?;
			let mut chapters: Vec<Chapter> = Vec::new();

			if let Some(items) = html.select("td[style='font-size:11pt;']") {
				let items_vec: Vec<_> = items.collect();
				if let Some(item) = items_vec.get(3) {
					if let Some(links) = item.select("td a") {
						for (index, link) in links.enumerate() {
							let chapter_key = link
								.attr("href")
								.unwrap_or_default()
								.split("/")
								.map(|a| a.to_string())
								.filter(|a| !a.is_empty())
								.collect::<Vec<String>>()
								.pop()
								.unwrap_or_default()
								.replace(".html", "");
							let title = link.text().unwrap_or_default().trim().to_string();
							let chapter_url = format!(
								"{}/m/comic/{}.html",
								WWW_URL, chapter_key
							);
							chapters.push(Chapter {
								key: chapter_key,
								title: Some(title),
								chapter_number: Some((index + 1) as f32),
								url: Some(chapter_url),
								..Default::default()
							});
						}
					}
				}
			}
			chapters.reverse();
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{}/m/comic/{}.html", WWW_URL, chapter.key);
		let html = parse_big5_html(&url)?;
		let img_url = handle_img_url(
			html.select_first("img[onload]")
				.and_then(|e| e.attr("src"))
				.unwrap_or_default(),
		);

		let mut length = 0i32;
		if let Some(items) = html.select(".pages:not(:has(img))") {
			if let Some(last) = items.last() {
				length = last.text().unwrap_or_default().parse::<i32>().unwrap_or(0);
			}
		}

		let mut pages: Vec<Page> = Vec::new();
		for index in 0..length {
			let page_url = img_url.replace("001.", &format!("{:03}.", index + 1));
			pages.push(Page {
				content: PageContent::url(page_url),
				..Default::default()
			});
		}

		Ok(pages)
	}
}

impl ListingProvider for CartoonmadSource {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let act = match listing.id.as_str() {
			"最新上架" => "1",
			"热门连载" => "2",
			_ => return self.get_search_manga_list(None, page, Vec::new()),
		};
		let url = format!("{}/m/?act={}&page={}", WWW_URL, act, page);
		let html = Request::get(&url)?.header("User-Agent", UA).html()?;
		parse_manga_list(html)
	}
}

impl ImageRequestProvider for CartoonmadSource {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		Ok(Request::get(&url)?.header("Referer", WWW_URL))
	}
}

register_source!(CartoonmadSource, ListingProvider, ImageRequestProvider);
