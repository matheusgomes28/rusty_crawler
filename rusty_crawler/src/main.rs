use anyhow::{Result, anyhow, bail};
use clap::Parser;
use url::Url;
use std::{process, collections::{VecDeque, HashSet}};
use html_parser::{Dom, Node, Element};

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct ProgramArgs {
    /// Name of the person to greet
    #[arg(short, long)]
    starting_url: String,
}

/// This will turn relative urls into
/// full urls.
/// E.g. get_url("/services/", "https://google.com/") -> "https://google.com/service/"
fn get_url(path: &str, root_url: Url) -> Result<Url> {
    match Url::parse(&path) {
        Ok(url) => Ok(url),
        _ => {
            match root_url.join(path) {
                Ok(url) => Ok(url),
                _ => bail!("could not join relative path")
            }
        }
    }
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

async fn crawl_url(url: Url) -> Result<Vec<String>>
{
    // Parsing html into a DOM obj
    let html = reqwest::get(url.clone())
        .await?
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

async fn try_main(args: ProgramArgs) -> Result<()> {

    let max_links = 1000;

    // Already visited links
    let mut already_visited: HashSet<String> = HashSet::new();

    // Another arg -> max number links
    let mut link_queue: VecDeque<String> = VecDeque::with_capacity(max_links);
    link_queue.push_back(args.starting_url);

    // Crawler loop
    'crawler: loop {
        // also check that max links have been reached
        if link_queue.is_empty() || (already_visited.len() > max_links) {
            break 'crawler;
        }

        // current url to visit
        let url_str = link_queue
            .pop_back()
            .ok_or_else(|| anyhow!("queue is empty"))?;

        let url = Url::parse(&url_str)?;

        let links = crawl_url(url)
            .await?;

        for link in links {
            if !already_visited.contains(&link) {
                link_queue.push_back(link)
            }
        }

        // add visited link to set of already visited link
        already_visited.insert(url_str);
    }

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