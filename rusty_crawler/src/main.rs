use anyhow::Result;
use clap::Parser;
use futures::{stream::FuturesUnordered, Future, StreamExt};
use tokio::sync::RwLock;
use std::{process, sync::Arc, pin::Pin, time::Duration, collections::VecDeque};

mod crawler;
use crawler::CrawlerStateRef;

use crate::crawler::{CrawlerState, crawl};

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct ProgramArgs {
    /// Name of the person to greet
    #[arg(short, long)]
    starting_url: String,

    /// Maximum links to find
    #[arg(short, long, default_value_t = 100)]
    max_links: u64
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
        max_links: args.max_links as usize,
    };
    let crawler_state = Arc::new(crawler_state);

    // The actual crawling goes here
    let mut tasks = FuturesUnordered::<Pin<Box<dyn Future<Output = Result<()>>>>>::new();
    tasks.push(Box::pin(crawl(crawler_state.clone(), 1)));
    tasks.push(Box::pin(crawl(crawler_state.clone(), 2)));
    tasks.push(Box::pin(crawl(crawler_state.clone(), 3)));
    tasks.push(Box::pin(crawl(crawler_state.clone(), 4)));
    //tasks.push(Box::pin(output_status(crawler_state.clone())));

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