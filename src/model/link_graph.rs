use std::collections::HashMap;

use serde::ser::{SerializeSeq, Serializer};

use super::{LinkId, Link};

#[derive(Debug)]
struct LinkGraph {
    links: HashMap<LinkId, Link>,   
    link_ids: HashMap<String, LinkId>,
}

impl LinkGraph {
    pub fn 
}
