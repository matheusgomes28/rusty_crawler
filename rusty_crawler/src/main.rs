use anyhow::{Result, anyhow, bail};
use clap::Parser;
use futures::{stream::FuturesUnordered, Future, StreamExt};
use reqwest::{StatusCode, Client};
use tokio::sync::RwLock;
use url::Url;
use std::{process, collections::{VecDeque, HashSet}, sync::Arc, pin::Pin, time::Duration};
use html_parser::{Dom, Node, Element};

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct ProgramArgs {
    /// Name of the person to greet
    #[arg(short, long)]
    starting_url: String,
}


struct CrawlerState {
    link_queue: RwLock<VecDeque<String>>,
    already_visited: RwLock<HashSet<String>>,
    max_links: usize,
}

type CrawlerStateRef = Arc<CrawlerState>;

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

fn is_node(node: &Node) -> bool {
    match node {
        Node::Element(..) => true,
        _ => false
    }
}

fn crawl_element(elem: &Element, root_url: Url) -> Result<Vec<String>> {

    let mut links: Vec<String> = Vec::new();

    // Figure out whether we have a link on this node!
    if elem.name == "a" {
        let href_attrib = elem
            .attributes
            .get("href")
            .ok_or_else(|| anyhow!("could not find href in link"))?
            .as_ref()
            .ok_or_else(|| anyhow!("href does not have a value"))?
            .clone();

        links.push(get_url(&href_attrib, root_url.clone())?.to_string());
    }

    for node in elem
        .children
        .iter()
        .filter(|c| is_node(c)) {
        
        match node {
            Node::Element(elem) => {
                // add whatever links from this elem to our vector
                let mut children_links = crawl_element(elem, root_url.clone())?;
                links.append(&mut children_links);
            },
            _ => {}
        }
    }

    Ok(links)
}

async fn find_links(url: Url, client: &Client) -> Result<Vec<String>>
{
    // Parsing html into a DOM obj
    log::info!("Finding links inside {}", url.as_str());


    let response = client
        .get(url.clone())
        .timeout(Duration::from_millis(10000))
        .send()
        .await?;

    if response.status() != StatusCode::OK {
        bail!("page returned invalid response");
    }

    let html = response
        .text()
        .await?;
    let dom = Dom::parse(&html)?;

    // Return links
    let mut res: Vec<String> = Vec::new();

    // Crawls all the nodes in the main html
    for child in dom.children {
        match child {
            Node::Element(elem) => {
                let links = match crawl_element(&elem, url.clone()) {
                    Ok(links) => links,
                    Err(e) => {
                        log::error!("Error: {}", e);
                        Vec::new()
                    }
                };

                for link in links {
                    res.push(link.clone());
                    log::info!("Link found in {}: {:?}", url, link);
                }
            },
            _ => {}
        }
    }
    
    Ok(res)
}

async fn crawl(crawler_state: CrawlerStateRef, worker_n: i32) -> Result<()> {
    // one client per worker thread
    let client = Client::new();

    // Crawler loop
    'crawler: loop {
        // also check that max links have been reached
        let mut link_queue = crawler_state.link_queue.write().await;
        let already_visited = crawler_state.already_visited.read().await;

        if link_queue.is_empty() {
            log::info!("Waiting for the next link from {}", worker_n);
            tokio::time::sleep(Duration::from_millis(500)).await;
            continue;
        }

        // If max links are reached, exit the worker thread
        if already_visited.len() > crawler_state.max_links {
            break 'crawler;
        }

        // current url to visit
        log::info!("Acquiring the next link from {}", worker_n);
        let url_str = link_queue
            .pop_back()
            .ok_or_else(|| anyhow!("queue is empty"))?; // SHould not exit the worker
        drop(link_queue);
        drop(already_visited);

        log::info!("!!!! finding links for {}", &url_str);
        let url = Url::parse(&url_str)?;
        let links = find_links(url, &client)
            .await?;

        
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

async fn output_status(crawler_state: CrawlerStateRef) -> Result<()> {
    loop {
        let already_visited = crawler_state.already_visited.read().await;
        log::info!("Number of links visited: {}", already_visited.len());
        for link in already_visited.iter() {
            log::info!("Already Visited: {}", link);
        }
        drop(already_visited);

        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}

async fn try_main(args: ProgramArgs) -> Result<()> {

    // call crawl(...)
    let crawler_state = CrawlerState{
        link_queue: RwLock::new(VecDeque::from([args.starting_url])),
        already_visited: RwLock::new(Default::default()),
        max_links: 1000,
    };
    let crawler_state = Arc::new(crawler_state);

    // The actual crawling goes here
    let mut tasks = FuturesUnordered::<Pin<Box<dyn Future<Output = Result<()>>>>>::new();
    tasks.push(Box::pin(crawl(crawler_state.clone(), 1)));
    tasks.push(Box::pin(crawl(crawler_state.clone(), 2)));
    tasks.push(Box::pin(crawl(crawler_state.clone(), 3)));
    tasks.push(Box::pin(crawl(crawler_state.clone(), 4)));
    tasks.push(Box::pin(output_status(crawler_state.clone())));

    while let Some(result) = tasks.next().await {
        match result {
            Err(e) => {
                log::error!("Error: {:?}", e);
            },
            _ => ()
        }
    }

    let already_visited = crawler_state.already_visited.read().await;
    println!("{:?}", already_visited);
    Ok(())
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let args = ProgramArgs::parse();

    match try_main(args).await {
        Ok(_) => {
            log::info!("Finished");
        },
        Err(e) => {
            log::error!("Error: {:?}", e);
            process::exit(-1);
        }
    }
}