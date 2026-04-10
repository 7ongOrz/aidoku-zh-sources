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
use regex::Regex;

const BASE_URL: &str = "https://www.bilimanga.net";
const UA: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 16_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.6 Mobile/15E148 Safari/604.1";

const FILTER_ORDER: [&str; 10] = [
	"weekvisit",
	"monthvisit",
	"weekvote",
	"monthvote",
	"weekflower",
	"monthflower",
	"words",
	"goodnum",
	"lastupdate",
	"postdate",
];

fn gen_request(url: &str) -> Result<Request> {
	Ok(Request::get(url)?
		.header("Origin", BASE_URL)
		.header("User-Agent", UA)
		.header("Accept-Language", "zh-CN,zh;q=0.9")
		.header("Cookie", "night=0"))
}

fn extract_chapter_number(title: &str) -> Option<f32> {
	let re = Regex::new(r"(?:第\s*)([\d\uFF10-\uFF19]+(?:\.[\d\uFF10-\uFF19]+)?)|([\d\uFF10-\uFF19]+(?:\.[\d\uFF10-\uFF19]+)?)\s*(?:话|話|章|回|卷|册|冊)").unwrap();
	if let Some(captures) = re.captures(title) {
		let num_match = captures.get(1).or_else(|| captures.get(2));
		if let Some(num_match) = num_match {
			let num_str = num_match.as_str()
				.chars()
				.map(|c| match c {
					'\u{FF10}' => '0',
					'\u{FF11}' => '1',
					'\u{FF12}' => '2',
					'\u{FF13}' => '3',
					'\u{FF14}' => '4',
					'\u{FF15}' => '5',
					'\u{FF16}' => '6',
					'\u{FF17}' => '7',
					'\u{FF18}' => '8',
					'\u{FF19}' => '9',
					'\u{FF0E}' => '.',
					other => other,
				})
				.collect::<String>();
			if let Ok(num) = num_str.parse::<f32>() {
				return Some(num);
			}
		}
	}
	None
}

fn parse_manga_items(html: &aidoku::imports::html::Document) -> Vec<Manga> {
	let mut entries: Vec<Manga> = Vec::new();
	if let Some(items) = html.select(".book-li>a") {
		for item in items {
			let key = item
				.attr("href")
				.unwrap_or_default()
				.split("/")
				.map(|a| a.to_string())
				.filter(|a| !a.is_empty())
				.collect::<Vec<String>>()
				.pop()
				.unwrap_or_default()
				.replace(".html", "");
			let cover = item
				.select_first(".book-cover>img")
				.and_then(|e| e.attr("data-src"));
			let title = item
				.select_first(".book-title")
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
	entries
}

struct BilicomicSource;

impl Source for BilicomicSource {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut tagid = String::from("0");
		let mut sortid = String::from("0");
		let mut rgroupid = String::from("0");
		let mut order = String::from("lastupdate");
		let mut anime = String::from("0");
		let mut quality = String::from("0");
		let mut isfull = String::from("0");
		let mut update = String::from("0");

		for filter in filters {
			match filter {
				FilterValue::Select { id, value } => match id.as_str() {
					"tagid" => tagid = value,
					"sortid" => sortid = value,
					"rgroupid" => rgroupid = value,
					"anime" => anime = value,
					"quality" => quality = value,
					"isfull" => isfull = value,
					"update" => update = value,
					_ => {}
				},
				FilterValue::Sort { id, index, .. } => {
					if id == "order" {
						order = FILTER_ORDER
							.get(index as usize)
							.unwrap_or(&"lastupdate")
							.to_string();
					}
				}
				_ => {}
			}
		}

		let url = if let Some(ref q) = query {
			format!(
				"{}/search/{}_{}.html",
				BASE_URL,
				encode_uri(q.clone()),
				page
			)
		} else {
			format!(
				"{}/filter/{}_{}_{}_{}_{}_{}_{}_{}_{}_0.html",
				BASE_URL, order, tagid, isfull, anime, rgroupid, sortid, update, quality, page
			)
		};

		let html = gen_request(&url)?
			.header("Referer", &format!("{}/search.html", BASE_URL))
			.html()?;

		// Check pagination
		let has_next_page = if query.is_none() {
			let strong_text = html
				.select_first("#pagelink strong")
				.and_then(|e| e.text())
				.unwrap_or_default();
			let last_text = html
				.select_first("#pagelink .last")
				.and_then(|e| e.text())
				.unwrap_or_default();
			strong_text != last_text
		} else {
			let next_href = html
				.select_first("#pagelink .next")
				.and_then(|e| e.attr("href"))
				.unwrap_or_default();
			next_href != "#"
		};

		// Check if it's a single manga detail redirect
		let alternate_url = html
			.select_first("link[rel='alternate']")
			.and_then(|e| e.attr("href"))
			.unwrap_or_default();

		let entries = if alternate_url.contains("detail") {
			let key = alternate_url
				.split("/")
				.map(|a| a.to_string())
				.filter(|a| !a.is_empty())
				.collect::<Vec<String>>()
				.pop()
				.unwrap_or_default()
				.replace(".html", "");
			let cover = html
				.select_first(".book-cover")
				.and_then(|e| e.attr("src"));
			let title = html
				.select_first("h1.book-title")
				.and_then(|e| e.text())
				.unwrap_or_default();
			aidoku::alloc::vec![Manga {
				key,
				cover,
				title,
				..Default::default()
			}]
		} else {
			parse_manga_items(&html)
		};

		Ok(MangaPageResult {
			has_next_page,
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
			let url = format!("{}/detail/{}.html", BASE_URL, manga.key);
			let html = gen_request(&url)?.html()?;

			manga.cover = html
				.select_first(".book-cover")
				.and_then(|e| e.attr("src"));
			manga.title = html
				.select_first("h1.book-title")
				.and_then(|e| e.text())
				.unwrap_or_default();

			let mut authors: Vec<String> = Vec::new();
			if let Some(author_nodes) = html.select(".authorname,.illname") {
				for a in author_nodes {
					if let Some(t) = a.text() {
						authors.push(t);
					}
				}
			}
			manga.authors = Some(authors);

			manga.description = html
				.select_first(".book-summary>content")
				.and_then(|e| e.text());

			let mut tags: Vec<String> = Vec::new();
			if let Some(tag_nodes) = html.select(".tag-small-group>.tag-small>a") {
				for t in tag_nodes {
					if let Some(text) = t.text() {
						tags.push(text);
					}
				}
			}
			manga.tags = Some(tags);

			let layout_text = html
				.select_first(".book-layout-inline")
				.and_then(|e| e.text())
				.unwrap_or_default();
			let status_str = layout_text
				.trim()
				.split("|")
				.next()
				.unwrap_or("")
				.trim();
			manga.status = match status_str {
				"\u{9023}\u{8F09}" => MangaStatus::Ongoing,
				"\u{5B8C}\u{7D50}" => MangaStatus::Completed,
				_ => MangaStatus::Unknown,
			};
			manga.content_rating = ContentRating::Safe;
			manga.viewer = Viewer::RightToLeft;
			manga.url = Some(url);
		}

		if needs_chapters {
			let url = format!("{}/read/{}/catalog", BASE_URL, manga.key);
			let html = gen_request(&url)?.html()?;
			let mut chapters: Vec<Chapter> = Vec::new();

			if let Some(volumes) = html.select(".catalog-volume") {
				for volume in volumes {
					let volume_title = volume
						.select_first("h3")
						.and_then(|e| e.text())
						.unwrap_or_default();
					let volume_num = extract_chapter_number(&volume_title).unwrap_or(-1.0);

					// Check if chapter links contain javascript
					let mut has_javascript_link = false;
					if let Some(links) = volume.select(".chapter-li-a") {
						for link in links {
							let href = link.attr("href").unwrap_or_default();
							if href.starts_with("javascript:") {
								has_javascript_link = true;
								break;
							}
						}
					}

					if has_javascript_link {
						let vol_href = volume
							.select_first(".volume-cover-img")
							.and_then(|e| e.attr("href"))
							.unwrap_or_default();
						let vol_url = format!("{}{}", BASE_URL, vol_href);
						let vol_html = gen_request(&vol_url)?.html()?;

						if let Some(chapter_items) = vol_html.select(".catalog-volume .chapter-li-a") {
							for chapter_item in chapter_items {
								let chapter_href = chapter_item
									.attr("href")
									.unwrap_or_default();
								let chapter_key = chapter_href
									.split("/")
									.map(|a| a.to_string())
									.filter(|a| !a.is_empty())
									.collect::<Vec<String>>()
									.pop()
									.unwrap_or_default()
									.replace(".html", "");
								let title = chapter_item
									.select_first("span")
									.and_then(|e| e.text())
									.unwrap_or_default();
								let chapter_num = extract_chapter_number(&title)
									.unwrap_or(chapters.len() as f32 + 1.0);
								let ch_url = format!("{}{}", BASE_URL, chapter_href);
								chapters.push(Chapter {
									key: chapter_key,
									title: Some(title),
									volume_number: Some(volume_num),
									chapter_number: Some(chapter_num),
									url: Some(ch_url),
									..Default::default()
								});
							}
						}
					} else if let Some(links) = volume.select(".chapter-li-a") {
						for item in links {
							let chapter_href = item
								.attr("href")
								.unwrap_or_default();
							let chapter_key = chapter_href
								.split("/")
								.map(|a| a.to_string())
								.filter(|a| !a.is_empty())
								.collect::<Vec<String>>()
								.pop()
								.unwrap_or_default()
								.replace(".html", "");
							let title = item
								.select_first("span")
								.and_then(|e| e.text())
								.unwrap_or_default();
							let chapter_num = extract_chapter_number(&title)
								.unwrap_or(chapters.len() as f32 + 1.0);
							let ch_url = format!("{}{}", BASE_URL, chapter_href);
							chapters.push(Chapter {
								key: chapter_key,
								title: Some(title),
								chapter_number: Some(chapter_num),
								volume_number: Some(volume_num),
								url: Some(ch_url),
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

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!(
			"{}/read/{}/{}.html",
			BASE_URL, manga.key, chapter.key
		);
		let html = gen_request(&url)?.html()?;
		let mut pages: Vec<Page> = Vec::new();

		if let Some(items) = html.select("#acontentz>img") {
			for item in items {
				let img_url = item.attr("data-src").unwrap_or_default().trim().to_string();
				pages.push(Page {
					content: PageContent::url(img_url),
					..Default::default()
				});
			}
		}

		Ok(pages)
	}
}

impl ListingProvider for BilicomicSource {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let name = match listing.id.as_str() {
			"月点击榜" => "monthvisit",
			"周点击榜" => "weekvisit",
			"月推荐榜" => "monthvote",
			"周推荐榜" => "weekvote",
			"月鲜花榜" => "monthflower",
			"周鲜花榜" => "weekflower",
			"月鸡蛋榜" => "monthegg",
			"周鸡蛋榜" => "weekegg",
			"最近更新" => "lastupdate",
			"最新入库" => "postdate",
			"收藏榜" => "goodnum",
			"新书榜" => "newhot",
			_ => return self.get_search_manga_list(None, page, Vec::new()),
		};

		let url = format!("{}/top/{}/1.html", BASE_URL, name);
		let html = gen_request(&url)?.html()?;
		let entries = parse_manga_items(&html);
		Ok(MangaPageResult {
			has_next_page: false,
			entries,
		})
	}
}

impl ImageRequestProvider for BilicomicSource {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		Ok(Request::get(&url)?
			.header("User-Agent", UA)
			.header("Referer", BASE_URL))
	}
}

register_source!(BilicomicSource, ListingProvider, ImageRequestProvider);
