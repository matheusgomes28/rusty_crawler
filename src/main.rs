use anyhow::Result;
use clap::Parser;
use log2::*;
use logger::spinner::Colour;
use model::LinkGraph;
use reqwest::Client;
use std::{collections::VecDeque, process, sync::Arc, time::Duration};
use tokio::{fs, sync::RwLock, task::JoinSet};
use url::Url;

mod crawler;
mod image_utils;
mod logger;
mod model;
use crawler::{scrape_page, CrawlerStateRef, LinkPath, ScrapeOption};

use crate::{
    crawler::CrawlerState,
    image_utils::{convert_links_to_images, download_images},
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

async fn output_status(crawler_state: CrawlerStateRef, total_links: u64) -> Result<()> {
    let progress_bar = logger::progress_bar::ProgressBar::new(total_links);
    progress_bar.message("Finding links");
    'output: loop {
        let link_queue = crawler_state.link_queue.read().await;
        let link_graph = crawler_state.link_graph.read().await;

        if link_graph.len() > crawler_state.max_links {
            // Show the links
            info!("All links found: {:#?}", link_graph);
            break 'output;
        }

        progress_bar.set_step(link_graph.len() as u64);

        drop(link_queue);
        drop(link_graph);

        tokio::time::sleep(Duration::from_millis(500)).await;
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
                info!("Link already found: {}", &link);
            }
        }

        if let Err(e) = link_graph.update(
            &child,
            &parent,
            &scrape_output.links,
            &scrape_output.images,
            &scrape_output.titles,
        ) {
            error!("could not update the link graph with {:#?}", e);
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
            output_status(crawler_state.clone(), args.max_links).await
        }));
    }

    while let Some(result) = tasks.join_next().await {
        if let Err(e) = result {
            error!("Error: {:?}", e);
        }
    }
    // FINISHED CRAWLING

    let link_graph = crawler_state.link_graph.read().await;

    let spinner = logger::spinner::Spinner::new();
    spinner.status("[1/4] converting image links");
    let image_metadata = convert_links_to_images(&link_graph);
    spinner.print_above("  [1/4] converted image links", Colour::Green);

    spinner.status("[2/4] downloading image metadata");
    download_images(&image_metadata, &args.img_save_dir, args.max_images).await?;
    spinner.print_above("  [2/4] downloaded image metadata", Colour::Green);

    // Save this to image dir
    spinner.status("[3/4] creating image database");
    let image_database = serde_json::to_string(&image_metadata)?;
    fs::write(args.img_save_dir + "database.json", image_database).await?;
    spinner.print_above("  [3/4] created image database", Colour::Green);

    spinner.status(format!("[4/4] serializing links to {}", args.links_json));
    serialize_links(&link_graph, &args.links_json).await?;
    spinner.print_above(
        format!("  [4/4] serializing links to {}", args.links_json),
        Colour::Green,
    );

    Ok(())
}

fn pretty_print_args(args: &ProgramArgs) {
    println!(
        "{}",
        console::style("CRAWLER INPUT ARGUMENTS").white().on_black()
    );
    println!(
        "{}  Starting URL: {}",
        console::Emoji("🌐", ""),
        console::style(&args.starting_url).bold().cyan()
    );
    println!(
        "{}  Maximum visited links: {}",
        console::Emoji("🔗", ""),
        console::style(&args.max_links).bold().cyan()
    );
    println!(
        "{}  Maximum number of images: {}",
        console::Emoji("🖼️", ""),
        console::style(&args.max_images).bold().cyan()
    );
    println!(
        "{}  Number of workers: {}",
        console::Emoji("⚒️", ""),
        console::style(&args.n_worker_threads).bold().cyan()
    );
    println!(
        "{}  Should log progress? {}",
        console::Emoji("❔", ""),
        console::style(args.log_status).bold().cyan()
    );
    println!(
        "{}  Image directory: {}",
        console::Emoji("📁", ""),
        console::style(&args.img_save_dir).bold().cyan()
    );
    println!(
        "{}  Output json path: {}",
        console::Emoji("📁", ""),
        console::style(&args.links_json).bold().cyan()
    );
    println!()
}

#[tokio::main]
async fn main() {
    let _log2 = log2::open("log.txt");

    // Print the arguments passed in nicely
    let args = ProgramArgs::parse();
    pretty_print_args(&args);

    match try_main(args).await {
        Ok(_) => {
            println!(
                "{} {}",
                console::Emoji("✅", ""),
                console::style("Finished!").green()
            );
        }
        Err(e) => {
            error!("Error: {:?}", e);
            process::exit(-1);
        }
    }
}
