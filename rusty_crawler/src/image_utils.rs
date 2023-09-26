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

use anyhow::{Result, anyhow, bail};
use url::Url;
use std::collections::HashMap;
use std::path::Path;

use reqwest::Client;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio_stream::StreamExt;
use uuid::Uuid;

use crate::common::Image;


/// Convert all the images in the input to
/// our json format
pub fn convert_to_json(images: &[Image]) -> HashMap<String, Image> {
    
    let mut image_map: HashMap<String, Image> = HashMap::new();

    for image in images {
        let uuid = Uuid::new_v4().to_string();
        image_map.insert(uuid, image.clone());
    }
    
    image_map
}

/// This function downloads one image into the destination
/// using the tokio stream io extensions. Note that this
/// contains modified code from https://gist.github.com/giuliano-oliveira/4d11d6b3bb003dba3a1b53f43d81b30d
async fn download_image(link: &str, destination: &str, client: &Client) -> Result<()> {
    // Download the image
    let res = client
        .get(link)
        .send()
        .await?;

    let mut file = File::create(destination).await?;
    let mut stream = res.bytes_stream();

    // download chunks
    while let Some(item) = stream.next().await {
        let chunk = item?;
        file.write_all(&chunk).await?;
    }

    Ok(())
}

/// Takes in the hashmap (image name, image info), downloads the images
/// and saves them to disk.
pub async fn download_images(images: HashMap<String, Image>, save_directory: &str, client: &Client) -> Result<()> {
    let directory_path = Path::new(&save_directory);
    if !directory_path.is_dir() {
        bail!("given save directory is invalid");
    }

    for (name, image) in images {
        // directory + name + extension
        let original_name = Url::parse(&image.link)?;
        if let Some(image_name) = original_name
            .path_segments()
            .iter()
            .last()
            .map(|s| s.clone().collect::<String>()){
            

            let name_ext = name + "." + image_name.split('.').last().unwrap_or("unknown");
            let destination_path = directory_path.join(name_ext);
            let destination = destination_path
                .to_str()
                .ok_or_else(|| anyhow!("could not get destination path"))?;

            download_image(&image.link, &destination, &client).await?;
            continue;
        }
        
        log::error!("could not get filename for image {}", image.link);
    }

    Ok(())
}


#[cfg(test)]
mod tests {
    use crate::common::Image;

    use super::convert_to_json;

    #[test]
    fn convert_to_json_empty() {
        let images = vec![
            Image{link: "path1.jpg".to_string(), alt: "some alt 1".to_string()},
            Image{link: "path2.jpg".to_string(), alt: "".to_string()},
        ];

        let image_map = convert_to_json(&images);
        let result = serde_json::to_string(&image_map);
        print!("Result: {:#?}", result);
    }
}