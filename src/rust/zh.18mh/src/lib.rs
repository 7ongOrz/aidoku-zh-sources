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

const WWW_URL: &str = "https://18mh.org";

const FILTER_GENRE: [&str; 4] = ["hanman", "zhenrenxiezhen", "riman", "aixiezhen"];

struct MhSource;

fn parse_manga_list(html: aidoku::imports::html::Document) -> Result<MangaPageResult> {
	let mut entries: Vec<Manga> = Vec::new();

	if let Some(items) = html.select(".pb-2>a") {
		for item in items {
			let key = item
				.attr("href")
				.unwrap_or_default()
				.split("/")
				.map(|a| a.to_string())
				.collect::<Vec<String>>()
				.pop()
				.unwrap_or_default();
			let cover = item
				.select_first("div>img")
				.and_then(|e| e.attr("src"));
			let title = item
				.select_first("div>h3")
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

	Ok(MangaPageResult {
		has_next_page: !entries.is_empty(),
		entries,
	})
}

impl Source for MhSource {
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
				if id == "category" {
					category = value;
				}
			}
		}

		let url = if let Some(query) = query {
			format!("{}/s/{}?page={}", WWW_URL, encode_uri(query), page)
		} else {
			let category_str = if category.is_empty() {
				String::from("manga")
			} else if FILTER_GENRE.contains(&category.as_str()) {
				format!("manga-genre/{}", category)
			} else {
				format!("manga-tag/{}", category)
			};
			format!("{}/{}/page/{}", WWW_URL, category_str, page)
		};

		let html = Request::get(&url)?.html()?;
		parse_manga_list(html)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let ids: Vec<String> = manga.key.split('/').map(|s| s.to_string()).collect();
		let manga_id = ids.first().cloned().unwrap_or_default();
		let mut mid = ids.get(1).cloned().unwrap_or_default();
		let should_fetch_manga_page = needs_details || (needs_chapters && mid.is_empty());
		let mut details_html = None;

		if should_fetch_manga_page {
			let url = format!("{}/manga/{}", WWW_URL, manga_id);
			let html = Request::get(&url)?.html()?;
			let fetched_mid = html
				.select_first("#mangachapters")
				.and_then(|e| e.attr("data-mid"))
				.unwrap_or_default();
			if mid.is_empty() {
				mid = fetched_mid.clone();
			}
			details_html = Some((url, html));
		}

		if needs_details {
			let (url, html) = details_html.take().unwrap();
			manga.cover = html
				.select_first("meta[property='og:image']")
				.and_then(|e| e.attr("content"));
			manga.title = html
				.select_first("title")
				.and_then(|e| e.text())
				.unwrap_or_default()
				.replace("-18漫畫", "");
			let mut authors: Vec<String> = Vec::new();
			if let Some(author_list) = html.select("a[href*=author]>span") {
				for a in author_list {
					let name = a.text().unwrap_or_default().replace(",", "");
					if !name.is_empty() {
						authors.push(name);
					}
				}
			}
			manga.authors = Some(authors);
			manga.description = html
				.select_first(".text-medium.my-unit-md")
				.and_then(|e| e.text());
			let mut tags: Vec<String> = Vec::new();
			if let Some(tag_list) = html.select(".py-1>a:not([href*=author])>span") {
				for a in tag_list {
					let tag = a
						.text()
						.unwrap_or_default()
						.replace(",", "")
						.replace("熱門漫畫", "")
						.replace("#", "")
						.replace("推荐", "")
						.trim()
						.to_string();
					if !tag.is_empty() {
						tags.push(tag);
					}
				}
			}
			manga.tags = Some(tags);
			manga.status = match html
				.select_first("h1.mb-2>span")
				.and_then(|e| e.text())
				.unwrap_or_default()
				.trim()
			{
				"連載中" => MangaStatus::Ongoing,
				"完結" => MangaStatus::Completed,
				_ => MangaStatus::Unknown,
			};
			manga.content_rating = ContentRating::NSFW;
			manga.viewer = Viewer::Webtoon;
			manga.url = Some(url);
		}

		if !mid.is_empty() {
			manga.key = format!("{}/{}", manga_id, mid);
		}

		if needs_chapters {
			let url = format!("{}/manga/get?mid={}&mode=all", WWW_URL, mid);
			let html = Request::get(&url)?.html()?;
			let mut chapters: Vec<Chapter> = Vec::new();

			if let Some(list) = html.select("#allchapterlist>.chapteritem>a") {
				for (index, item) in list.enumerate() {
					let key = item.attr("data-cs").unwrap_or_default();
					let title = item
						.select_first("div>span:nth-child(1)")
						.and_then(|e| e.text())
						.unwrap_or_default()
						.trim()
						.to_string();
					let slug = item.attr("href").unwrap_or_default();
					let chapter_url = format!("{}/{}", WWW_URL, slug);
					chapters.push(Chapter {
						key,
						title: Some(title),
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
		let ids: Vec<&str> = manga.key.split("/").collect();
		let url = format!(
			"{}/chapter/getcontent?m={}&c={}",
			WWW_URL, ids[1], chapter.key
		);
		let html = Request::get(&url)?.header("Referer", WWW_URL).html()?;
		let mut pages: Vec<Page> = Vec::new();

		if let Some(list) = html.select("#chapcontent>div>img") {
			for item in list {
				let img_url = item
					.attr("data-src")
					.or_else(|| item.attr("src"))
					.unwrap_or_default();
				pages.push(Page {
					content: PageContent::url(img_url),
					..Default::default()
				});
			}
		}

		Ok(pages)
	}
}

impl ListingProvider for MhSource {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let list = match listing.id.as_str() {
			"人气推荐" => "hots",
			"热门更新" => "dayup",
			"最新上架" => "newss",
			_ => return self.get_search_manga_list(None, page, Vec::new()),
		};
		let url = format!("{}/{}/page/{}", WWW_URL, list, page);
		let html = Request::get(&url)?.html()?;
		parse_manga_list(html)
	}
}

impl ImageRequestProvider for MhSource {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		Ok(Request::get(&url)?.header("Referer", WWW_URL))
	}
}

register_source!(MhSource, ListingProvider, ImageRequestProvider);
