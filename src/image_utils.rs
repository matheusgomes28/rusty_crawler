/*
input:
    https://matgomes.com/path1.jpg
    path2.png
    ..
    path012931023.svg


-> download them to a directory
-> output json with info
{
    "uuid-qwe123-qwe123123.jpg": {
        "link": "https://matgomes.com/path1.jpg",
        "alt": "whatever text we have"
    },

    ...
}
*/

use anyhow::{anyhow, bail, Result};
use std::collections::HashMap;
use std::path::Path;

use reqwest::{Client, Response};
use tokio::fs::{create_dir, File};
use tokio::io::AsyncWriteExt;
use tokio_stream::StreamExt;
use uuid::Uuid;

use crate::model::{Image, LinkGraph};

/// Convert all the images in the found scraped
/// links to the (Uuid name, image) format
pub fn conver_links_to_images(links: &LinkGraph) -> HashMap<String, Image> {
    links
        .into_iter()
        .flat_map(|(_, link)| link.images.clone())
        .map(|img| (Uuid::new_v4().to_string(), img))
        .collect()
}

/// This function downloads one image into the destination
/// using the tokio stream io extensions. Note that this
/// contains modified code from https://gist.github.com/giuliano-oliveira/4d11d6b3bb003dba3a1b53f43d81b30d
/// destination - the path to the destination without the extension!
async fn download_image(link: &str, destination: &str, client: &Client) -> Result<()> {
    // Download the image
    let res = client.get(link).send().await?;

    // Get the content type here
    let extension = get_extension(&res)?;

    let mut file = File::create(destination.to_string() + "." + extension).await?;
    let mut stream = res.bytes_stream();

    // download chunks
    while let Some(item) = stream.next().await {
        let chunk = item?;
        file.write_all(&chunk).await?;
    }

    Ok(())
}

fn get_extension(res: &Response) -> Result<&str> {
    // Here where we can get the "content-type" and "image/gif"
    let content_type = res
        .headers()
        .get("content-type")
        .ok_or_else(|| anyhow!("failed to get content type"))?
        .to_str()?;

    match content_type {
        "image/gif" => Ok("gif"),
        "image/jpeg" => Ok("jpg"),
        "image/png" => Ok("png"),
        "image/svg+xml" => Ok("svg"),
        "image/webp" => Ok("webp"),
        "image/tiff" => Ok("tif"),
        _ => bail!("unsupported extension type"),
    }
}

/// Takes in the hashmap (image name, image info), downloads the images
/// and saves them to disk.
pub async fn download_images(
    images: &HashMap<String, Image>,
    save_directory: &str,
    client: &Client,
    max_links: u64,
) -> Result<()> {
    let directory_path = Path::new(&save_directory);
    if !directory_path.is_dir() {
        // bail!("given save directory is invalid");
        create_dir(directory_path).await?;
    }

    for (name, image) in images.iter().take(max_links as usize) {
        // directory + name + extension
        let destination_path = directory_path.join(name);
        let destination = destination_path
            .to_str()
            .ok_or_else(|| anyhow!("could not get destination path"))?;

        if let Err(e) = download_image(&image.link, destination, client).await {
            log::error!("Could not download image {}, error: {}", image.link, e);
        }
    }

    Ok(())
}

// #[cfg(test)]
// mod tests {
//     // use crate::model::Image;

//     #[test]
//     fn a_unit_test() {
//         assert!(true);
//     }
// }
