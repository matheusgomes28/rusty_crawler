use anyhow::{Result, anyhow};
use std::process;
use html_parser::{Dom, Node, Element};


/// This will turn relative urls into
/// full urls.
/// E.g. get_url("/services/", "https://google.com/") -> "https://google.com/service/"
fn get_url(url: &str, root_url: &str) -> String {
    if url.starts_with("https://") || url.starts_with("http://") {
        return url.into();
    }

    format!("{}/{}", root_url.strip_suffix('/').unwrap_or(root_url), url.strip_prefix('/').unwrap_or(url))
}

fn is_node(node: &Node) -> bool {
    match node {
        Node::Element(..) => true,
        _ => false
    }
}

fn crawl_element(elem: Element, root_url: &str) -> Result<Vec<String>> {

    let mut links: Vec<String> = Vec::new();

    // Figure out whether we have a link on this node!
    if elem.name == "a" {
        let href_attrib = elem
            .attributes
            .iter()
            .filter(|(name, _)| name.as_str() == "href")
            .last()
            .ok_or_else(|| anyhow!("no href found in a"));


        // Check if we have the "href"attribute
        match href_attrib {
            Ok((_key, Some(val))) => {
                links.push(get_url(val, &root_url));
            },
            _ => {
                log::error!("No link found for element {}", elem.name);
            }
        }
    }

    for node in elem
        .children
        .iter()
        .filter(|c| is_node(c)) {
        
        match node {
            Node::Element(elem) => {
                // add whatever links from this elem to our vector
                let mut children_links = crawl_element(elem.clone(), root_url)?;
                links.append(&mut children_links);
            },
            _ => {}
        }
    }

    Ok(links)
}

async fn crawl_url(url: &str) -> Result<Vec<String>>
{
    // Parsing html into a DOM obj
    let html = reqwest::get(url)
        .await?
        .text()
        .await?;
    let dom = Dom::parse(&html)?;

    // Crawls all the nodes in the main html
    for child in dom.children {
        match child {
            Node::Element(elem) => {

                for link in crawl_element(elem, url)? {
                    log::info!("Link found in {}: {:?}", url, link);
                }
            },
            _ => {}
        }
    }

    
    // TODO : change this to the sum of links
    let res: Vec<String> = Vec::new();
    Ok(res)
}

async fn try_main() -> Result<()> {

    let _ = crawl_url("https://google.com").await?;

    log::info!("hello world!");

    Ok(())
}

#[tokio::main]
async fn main() {
    env_logger::init();

    match try_main().await {
        Ok(_) => {
            log::info!("Finished");
        },
        Err(e) => {
            log::error!("Error: {:?}", e);
            process::exit(-1);
        }
    }
}