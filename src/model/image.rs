use serde::Serialize;


#[derive(Clone, Debug, Serialize)]
pub struct Image {
    /// the link for this image
    pub link: String,
    /// the alternative text found within the image
    pub alt: String,
}
