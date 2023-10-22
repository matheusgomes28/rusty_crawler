use anyhow::Result;
use clap::Parser;
use reqwest::Client;
use tokio::{sync::RwLock, task::JoinSet, fs};
use url::Url;
use std::{process, sync::Arc, time::Duration, collections::{VecDeque, HashMap}};

mod crawler;
mod image_utils;
mod model;
use crawler::{CrawlerStateRef, scrape_page, ScrapeOption};

use crate::{
    crawler::CrawlerState,
    image_utils::{download_images, conver_links_to_images},
    model::{Link, LinkId}
};

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct ProgramArgs {
    /// Name of the person to greet
    #[arg(short, long)]
    starting_url: String,

    /// Maximum links to find
    #[arg(long, default_value_t = 100)]
    max_links: u64,

    /// Max images
    #[arg(long, default_value_t = 100)]
    max_images: u64,

    /// Number of worker threads
    #[arg(short, long, default_value_t = 4)]
    n_worker_threads: u64,

    /// Enable logging the current status
    #[arg(short, long, default_value_t = false)]
    log_status: bool,

    /// The directory to save all the images scraped
    #[arg(short, long, default_value_t = String::from("images/"))]
    img_save_dir: String,

    /// The file to save the link information to
    #[arg(long, default_value_t = String::from("links.json"))]
    links_json: String
}

async fn output_status(crawler_state: CrawlerStateRef) -> Result<()> {
    'output: loop {
        let link_queue = crawler_state.link_queue.read().await;
        let visited_links = crawler_state.visited_links.read().await;
        // let images = crawler_state.images.read().await;

        if visited_links.len() > crawler_state.max_links {
            // Show the links
            println!("All links found: {:#?}", visited_links);
            // println!("All images found {:#?}", images);
            break 'output;
        }

        println!("Number of links visited: {}", visited_links.len());
        println!("Number of links in the queue: {}", link_queue.len());
        // println!("Number of images found: {}", images.len());

        drop(link_queue);
        drop(visited_links);
        // drop(images);

        tokio::time::sleep(Duration::from_secs(3)).await;
    }

    Ok(())
}

// Given a list of links (urls), this will return a list
// of all the found IDs. Links that have not been visited
// will be filtered out
fn find_link_ids(links: &[String], link_id_map: &HashMap<String, LinkId>) -> Vec<LinkId> {
    links
        .iter()
        .filter_map(|l| link_id_map.get(l))
        .cloned()
        .collect()
}

async fn crawl(crawler_state: CrawlerStateRef) -> Result<()> {
    // one client per worker thread
    let client = Client::new();

    // Crawler loop
    'crawler: loop {
        let visited_links = crawler_state.visited_links.read().await;
        if visited_links.len() > crawler_state.max_links {
            break 'crawler;
        }
        drop(visited_links);

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
        let scrape_options = vec![ScrapeOption::Images, ScrapeOption::Titles];
        let scrape_output = scrape_page(url, &client, &scrape_options).await;

        // TODO : find children already visited
        // TODO : find parents already visited

        // TODO : Analyse the performance when we have all these locks being
        // dropped multiple times
        let link_ids = crawler_state.link_ids.read().await;
        let link = Link::new(
            url_str.clone(),
            find_link_ids(&scrape_output.links, &link_ids),
            Default::default(), // Find a way to add parents here
            scrape_output.images,
            Default::default(),
        );
        drop(link_ids);
        
       
        let mut link_queue: tokio::sync::RwLockWriteGuard<'_, VecDeque<String>> = crawler_state.link_queue.write().await;
        let mut visited_links = crawler_state.visited_links.write().await;
        let mut link_ids = crawler_state.link_ids.write().await;
        for link in scrape_output.links {
            // TODO : check if we already have this link in the map
            //        if not, add to queue
            if let None = link_ids.get(&link){
                link_queue.push_back(link)
            }
        }

        // add visited link to set of already visited link
        // TODO : add the value (created link)
        let link_id = link.id;
        visited_links.insert(link_id, link);
        link_ids.insert(url_str,link_id);
    }

    Ok(())
}

async fn serialize_links(links: &HashMap<LinkId, Link>, destination: &str) -> Result<()>
{
    let json = serde_json::to_string(links)?;
    fs::write(destination, json).await?;
    Ok(())
}

async fn try_main(args: ProgramArgs) -> Result<()> {

    // call crawl(...)
    let crawler_state = CrawlerState{
        link_queue: RwLock::new(VecDeque::from([args.starting_url])),
        link_ids: RwLock::new(Default::default()),
        visited_links: RwLock::new(Default::default()),
        max_links: args.max_links as usize,
    };
    let crawler_state = Arc::new(crawler_state);

    // The actual crawling goes here
    let mut tasks = JoinSet::new();

    for _ in 0..args.n_worker_threads {
        let crawler_state = crawler_state.clone();
        let task = tokio::spawn(async move{
            crawl(crawler_state.clone()).await
        });

        tasks.spawn(task);
    }

    if args.log_status {
        let crawler_state = crawler_state.clone();
        tasks.spawn(tokio::spawn(async move {
            output_status(crawler_state.clone()).await
        }));
    }

    while let Some(result) = tasks.join_next().await {
        match result {
            Err(e) => {
                log::error!("Error: {:?}", e);
            },
            _ => ()
        }
    }

    let visited_links = crawler_state.visited_links.read().await;
    println!("{:?}", visited_links);

    let client = reqwest::Client::new();
    let visited_links = crawler_state.visited_links.read().await;
    let image_metadata = conver_links_to_images(&visited_links);
    download_images(&image_metadata, &args.img_save_dir, &client, args.max_images).await?;
    
    // Save this to image dir
    let image_database = serde_json::to_string(&image_metadata)?;
    fs::write(args.img_save_dir + "databse.json", image_database).await?;

    serialize_links(&visited_links, &args.links_json).await?;

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