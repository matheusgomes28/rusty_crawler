use anyhow::{Result, anyhow, bail};
use reqwest::{Client, StatusCode};
use scraper::Selector;
use url::Url;
use std::{collections::{VecDeque, HashSet}, sync::Arc, time::Duration};

use tokio::sync::RwLock;

const LINK_REQUEST_TIMEOUT_S: u64 = 2;

pub struct CrawlerState {
    pub link_queue: RwLock<VecDeque<String>>,
    pub already_visited: RwLock<HashSet<String>>,
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

    root_url.join(&path)
        .ok()
        .ok_or(anyhow!("could not join relative path"))
}

/// Given a `url` and a `client`, it will return the
/// parsed HTML in a DOM structure. It may return
/// an error if the request fails.
async fn get_all_links(url: Url, client: &Client) -> Result<Vec<String>> {
    let response = client
        .get(url.clone())
        .timeout(Duration::from_secs(LINK_REQUEST_TIMEOUT_S))
        .send()
        .await?;

    if response.status() != StatusCode::OK {
        bail!("page returned invalid response");
    }

    let html = response
        .text()
        .await?;

    
    let link_selector = Selector::parse("a").unwrap();
    Ok(scraper::Html::parse_document(&html)
        .select(&link_selector)
        .filter_map(|e| e.value().attr("href"))
        .map(|href| href.to_string())
        .collect())
}

/// Given a `url`, and a `client`, it will crawl
/// the HTML in `url` and find all the links in the
/// page, returning them as a vector of strings
async fn find_links(url: Url, client: &Client) -> Vec<String>
{
    // This will get all the "href" tags in all the anchors
    let links = match get_all_links(url.clone(), &client).await {
        Ok(links) => links,
        Err(e) => {
            log::error!("Could not find links: {}", e);
            Vec::new()
        }
    };

    // Turn all links into absolute links
    links
        .iter()
        .filter_map(|l| get_url(l, url.clone()).ok())
        .map(|url| url.to_string())
        .collect()
}

pub async fn crawl(crawler_state: CrawlerStateRef) -> Result<()> {
    // one client per worker thread
    let client = Client::new();

    // Crawler loop
    'crawler: loop {
        let already_visited = crawler_state.already_visited.read().await;
        if already_visited.len() > crawler_state.max_links {
            break 'crawler;
        }
        drop(already_visited);

        // also check that max links have been reached
        let mut link_queue = crawler_state.link_queue.write().await;
        let url_str = link_queue.pop_back().unwrap_or("".to_string());
        drop(link_queue);

        if url_str.is_empty() {
            tokio::time::sleep(Duration::from_millis(500)).await;
            continue;
        }

        // current url to visit
        let url = Url::parse(&url_str)?;

        // Log the errors
        let links = find_links(url, &client).await;
        
       
        let mut link_queue = crawler_state.link_queue.write().await;
        let mut already_visited = crawler_state.already_visited.write().await;
        for link in links {
            if !already_visited.contains(&link) {
                link_queue.push_back(link)
            }
        }

        // add visited link to set of already visited link
        already_visited.insert(url_str);
    }

    Ok(())
}
