use std::collections::HashMap;

use serde::ser::{SerializeSeq, Serializer};

use super::{LinkId, Link};

#[derive(Debug)]
struct LinkGraph {
    links: HashMap<LinkId, Link>,   
    link_ids: HashMap<String, LinkId>,
}

impl LinkGraph {

    // Update a link
    fn update(url: &str, from: &str, children: &[&str]) -> Result<()> {
        
        // Case 1: If we already have the link here,
        //         we simply need to add the children and
        //         the parent
        
        // Parent has ID already? Then add parent to link
        // Parent does not have ID? Ignore the parent

        // for each children:
            // Child has ID already? Then add child to Link
            // Child does not have ID? Then ignore child.



        // Case 2: If we don't have the link, create a new
        //         `Link` object with the children and the
        //         parent

        // Parent has ID already? Then add parent to link
        // Parent does not have ID? Ignore the parent

        // for each children:
            // Child has ID already? Then add child to Link
            // Child does not have ID? Then ignore child.
        
        Ok(())
    }

    // Get the ID for a link
}

impl Iterator for LinkGraph {
    type Item = Link;

    fn next(&mut self) -> Option<Self::Item> {
        links.next()
    }
}

trait LinkSerializer {
    fn serialize(links: LinkGraph) -> Result<()>;
}
