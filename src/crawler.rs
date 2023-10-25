use anyhow::{anyhow, bail, Result};
use reqwest::{Client, StatusCode};
use scraper::{Html, Selector};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::Duration,
};
use tokio::sync::RwLock;
use url::Url;

use crate::model::Image;
use crate::model::{Link, LinkId};

const LINK_REQUEST_TIMEOUT_S: u64 = 2;

/// Enum to represent data to scrape from
/// each link
pub enum ScrapeOption {
    /// Find any image link with the given
    /// extensions. E.g. `Image("jpg")`
    Images,
    Titles, // TODO Add support for page titles
}

/*
pub struct PageInfo {
    links: Vec<String>,
    images: Vec<Image>,
    titles: Vec<Title>,
}
*/

pub struct ScrapeOutput {
    pub links: Vec<String>,
    pub images: Vec<Image>,
    pub titles: Vec<String>,
}

pub struct CrawlerState {
    pub link_queue: RwLock<VecDeque<String>>,
    pub visited_links: RwLock<HashMap<LinkId, Link>>,
    pub link_ids: RwLock<HashMap<String, LinkId>>,
    pub max_links: usize,
}

pub type CrawlerStateRef = Arc<CrawlerState>;

/// This will turn relative urls into
/// full urls.
/// E.g. get_url("/services/", "https://google.com/") -> "https://google.com/service/"
fn get_url(path: &str, root_url: Url) -> Result<Url> {
    if let Ok(url) = Url::parse(&path) {
        return Ok(url);
    }

    root_url
        .join(&path)
        .ok()
        .ok_or(anyhow!("could not join relative path"))
}

// TODO : we're gonna need to know the ID of the URL
fn get_images(html_dom: &Html, root_url: &Url) -> Vec<Image> {
    let img_selector = Selector::parse("img[src]").unwrap();

    let image_links = html_dom
        .select(&img_selector)
        .filter(|e| e.value().attr("src").is_some())
        .map(|e| {
            (
                e.value().attr("src").unwrap(),
                e.value().attr("alt").unwrap_or(""),
            )
        })
        .map(|(link, alt)| Image {
            link: link.to_string(),
            alt: alt.to_string(),
        });

    let mut result: Vec<Image> = Default::default();
    for image in image_links {
        // TODO remove the clone by taking a reference
        if let Ok(absolute_url) = get_url(&image.link, root_url.clone()) {
            result.push(Image {
                link: absolute_url.to_string(),
                ..image
            });
            continue;
        }

        log::error!("failed to join url"); // TODO : better image
    }

    result
}

/// This function will scrape all the titles from
/// the given page's DOM -> title tags, h1, and h2 tags
fn get_titles(html_dom: &Html) -> Vec<String> {
    let mut titles: Vec<String> = Default::default();

    for tag in ["h1", "h2", "title"] {
        let title_selector = Selector::parse(tag).unwrap();

        titles.extend(
            html_dom
                .select(&title_selector)
                .map(|e| e.text().collect::<String>()),
        );
    }

    titles
}

/// Given a `url` and a `client`, it will parse the
/// HTML in a DOM structure, and scrape all the information
/// requested. It will find links by default.
/// It may return an error if the request fails.
async fn scrape_page_helper(
    url: Url,
    client: &Client,
    options: &[ScrapeOption],
) -> Result<ScrapeOutput> {
    let response = client
        .get(url.clone())
        .timeout(Duration::from_secs(LINK_REQUEST_TIMEOUT_S))
        .send()
        .await?;

    if response.status() != StatusCode::OK {
        bail!("page returned invalid response");
    }

    let html = response.text().await?;

    let html_dom = scraper::Html::parse_document(&html);

    let link_selector = Selector::parse("a").unwrap();
    let links: Vec<String> = html_dom
        .select(&link_selector)
        .filter_map(|e| e.value().attr("href"))
        .map(|href| href.to_string())
        .collect();

    // Now also want to get the scrape data
    let mut images: Vec<Image> = Vec::new();
    let mut titles: Vec<String> = Vec::new();
    for option in options {
        match option {
            ScrapeOption::Images => {
                images = get_images(&html_dom, &url);
            }
            ScrapeOption::Titles => {
                titles = get_titles(&html_dom);
            }
        }
    }

    Ok(ScrapeOutput {
        links,
        images,
        titles,
    })
}

/// Given a `url`, and a `client`, it will crawl
/// the HTML in `url` and find all the links in the
/// page, returning them as a vector of strings
pub async fn scrape_page(url: Url, client: &Client, options: &[ScrapeOption]) -> ScrapeOutput {
    // This will get all the "href" tags in all the anchors
    // TODO : Pass in the options
    let mut scrape_output = match scrape_page_helper(url.clone(), &client, options).await {
        Ok(output) => output,
        Err(e) => {
            log::error!("Could not find links: {}", e);
            ScrapeOutput {
                images: Default::default(),
                links: Default::default(),
                titles: Default::default(),
            }
        }
    };

    // Turn all links into absolute links
    scrape_output.links = scrape_output
        .links
        .iter()
        .filter_map(|l| get_url(l, url.clone()).ok())
        .map(|url| url.to_string())
        .collect();

    scrape_output
}
