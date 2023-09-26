use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Image {
    pub link: String,
    pub alt: String,
}
