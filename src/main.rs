use anyhow::Result;
use clap::Parser;
use model::LinkGraph;
use reqwest::Client;
use std::{collections::VecDeque, process, sync::Arc, time::Duration};
use tokio::{fs, sync::RwLock, task::JoinSet};
use url::Url;

mod crawler;
mod image_utils;
mod model;
use crawler::{scrape_page, CrawlerStateRef, LinkPath, ScrapeOption};

use crate::{
    crawler::CrawlerState,
    image_utils::{conver_links_to_images, download_images},
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
    links_json: String,
}

async fn output_status(crawler_state: CrawlerStateRef) -> Result<()> {
    'output: loop {
        let link_queue = crawler_state.link_queue.read().await;
        let link_graph = crawler_state.link_graph.read().await;

        if link_graph.len() > crawler_state.max_links {
            // Show the links
            log::info!("All links found: {:#?}", link_graph);
            break 'output;
        }

        log::info!("Number of links in the queue: {}", link_queue.len());
        log::info!("Number of links visited: {}", link_graph.len());

        drop(link_queue);
        drop(link_graph);

        tokio::time::sleep(Duration::from_secs(3)).await;
    }

    Ok(())
}

async fn crawl(crawler_state: CrawlerStateRef) -> Result<()> {
    // one client per worker thread
    let client = Client::new();

    // Crawler loop
    'crawler: loop {
        let number_links_found = crawler_state.link_graph.read().await.len();
        if number_links_found > crawler_state.max_links {
            break 'crawler;
        }

        // also check that max links have been reached
        let mut link_queue = crawler_state.link_queue.write().await;
        let LinkPath { parent, child } = link_queue.pop_back().unwrap_or(Default::default());
        drop(link_queue);

        // Log the errors
        let scrape_options = vec![ScrapeOption::Images, ScrapeOption::Titles];
        let scrape_output = scrape_page(Url::parse(&child)?, &client, &scrape_options).await;

        let mut link_queue = crawler_state.link_queue.write().await;
        let mut link_graph = crawler_state.link_graph.write().await;
        for link in scrape_output.links.iter() {
            if !link_graph.link_visited(link) {
                // Check if the link already visited
                link_queue.push_back(LinkPath {
                    parent: child.clone(),
                    child: link.clone(),
                })
            } else {
                log::info!("Link already found: {}", &link);
            }
        }

        if let Err(e) = link_graph.update(
            &child,
            &parent,
            &scrape_output.links,
            &scrape_output.images,
            &scrape_output.titles,
        ) {
            log::error!("could not update the link graph with {:#?}", e);
        }
    }

    Ok(())
}

async fn serialize_links(links: &LinkGraph, destination: &str) -> Result<()> {
    let json = serde_json::to_string(links)?;
    fs::write(destination, json).await?;
    Ok(())
}

fn new_crawler_state(starting_url: String, max_links: u64) -> CrawlerStateRef {
    let crawler_state = CrawlerState {
        link_queue: RwLock::new(VecDeque::from([LinkPath {
            child: starting_url,
            ..Default::default()
        }])),
        link_graph: RwLock::new(Default::default()),
        max_links: max_links as usize,
    };

    Arc::new(crawler_state)
}

async fn try_main(args: ProgramArgs) -> Result<()> {
    let crawler_state = new_crawler_state(args.starting_url, args.max_links);

    // The actual crawling goes here
    let mut tasks = JoinSet::new();

    // Add as many crawling workers as the user has specified
    for _ in 0..args.n_worker_threads {
        let crawler_state = crawler_state.clone();
        let task = tokio::spawn(async move { crawl(crawler_state.clone()).await });

        tasks.spawn(task);
    }

    if args.log_status {
        let crawler_state = crawler_state.clone();
        tasks.spawn(tokio::spawn(async move {
            output_status(crawler_state.clone()).await
        }));
    }

    while let Some(result) = tasks.join_next().await {
        if let Err(e) = result {
            log::error!("Error: {:?}", e);
        }
    }

    let link_graph = crawler_state.link_graph.read().await;

    let client = reqwest::Client::new();
    let image_metadata = conver_links_to_images(&link_graph);

    download_images(
        &image_metadata,
        &args.img_save_dir,
        &client,
        args.max_images,
    )
    .await?;

    // Save this to image dir
    let image_database = serde_json::to_string(&image_metadata)?;
    fs::write(args.img_save_dir + "database.json", image_database).await?;

    serialize_links(&link_graph, &args.links_json).await?;

    Ok(())
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let args = ProgramArgs::parse();

    match try_main(args).await {
        Ok(_) => {
            log::info!("Finished");
        }
        Err(e) => {
            log::error!("Error: {:?}", e);
            process::exit(-1);
        }
    }
}
