use anyhow::Result;
use clap::Parser;
use reqwest::Client;
use tokio::{sync::RwLock, task::JoinSet};
use url::Url;
use std::{process, sync::Arc, time::Duration, collections::VecDeque};

mod crawler;
mod image_utils;
mod common;
use crawler::{CrawlerStateRef, scrape_page, ScrapeOption};

use crate::{crawler::CrawlerState, image_utils::{convert_to_json, download_images}};

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct ProgramArgs {
    /// Name of the person to greet
    #[arg(short, long)]
    starting_url: String,

    /// Maximum links to find
    #[arg(short, long, default_value_t = 100)]
    max_links: u64,

    /// Number of worker threads
    #[arg(short, long, default_value_t = 4)]
    n_worker_threads: u64,

    /// Enable logging the current status
    #[arg(short, long, default_value_t = false)]
    log_status: bool
}

async fn output_status(crawler_state: CrawlerStateRef) -> Result<()> {
    'output: loop {
        let link_queue = crawler_state.link_queue.read().await;
        let already_visited = crawler_state.already_visited.read().await;
        let images = crawler_state.images.read().await;

        if already_visited.len() > crawler_state.max_links {
            // Show the links
            println!("All links found: {:#?}", already_visited);
            println!("All images found {:#?}", images);
            break 'output;
        }

        println!("Number of links visited: {}", already_visited.len());
        println!("Number of links in the queue: {}", link_queue.len());
        println!("Number of images found: {}", images.len());

        drop(link_queue);
        drop(already_visited);
        drop(images);

        tokio::time::sleep(Duration::from_secs(3)).await;
    }

    Ok(())
}

async fn crawl(crawler_state: CrawlerStateRef) -> Result<()> {
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
        let scrape_options = vec![ScrapeOption::Images, ScrapeOption::Titles];
        let scrape_output = scrape_page(url, &client, &scrape_options).await;
       
        let mut link_queue = crawler_state.link_queue.write().await;
        let mut already_visited = crawler_state.already_visited.write().await;
        for link in scrape_output.links {
            if !already_visited.contains(&link) {
                link_queue.push_back(link)
            }
        }

        let mut images = crawler_state.images.write().await;
        images.extend(scrape_output.images);

        // add visited link to set of already visited link
        already_visited.insert(url_str);
    }

    Ok(())
}

async fn try_main(args: ProgramArgs) -> Result<()> {

    // call crawl(...)
    let crawler_state = CrawlerState{
        link_queue: RwLock::new(VecDeque::from([args.starting_url])),
        already_visited: RwLock::new(Default::default()),
        images: RwLock::new(Default::default()),
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

    let already_visited = crawler_state.already_visited.read().await;
    println!("{:?}", already_visited);

    let client = reqwest::Client::new();
    let images = crawler_state.images.read().await;
    let image_metadata = convert_to_json(&images);
    download_images(image_metadata, "/home/pi/development/rusty_crawler/rusty_crawler/images", &client).await?;
    
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