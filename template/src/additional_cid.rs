use std::collections::{HashMap, HashSet};

use serde::{Serialize, Serializer};

use mail_core::Resource;
use mail_headers::header_components::ContentId;

pub struct AdditionalCIds<'a> {
    additional_resources: &'a [&'a HashMap<String, Resource>],
}

impl<'a> AdditionalCIds<'a> {
    /// Creates a new `AdditionalCIds` instance.
    ///
    /// All resources in the all hash maps have to be loaded to the
    /// `Data` or `EncData` variants or using `get` can panic.
    pub(crate) fn new(additional_resources: &'a [&'a HashMap<String, Resource>]) -> Self {
        AdditionalCIds {
            additional_resources,
        }
    }

    /// Returns the content id associated with the given name.
    ///
    /// If multiple of the maps used to create this type contain the
    /// key the first match is returned and all later ones are ignored.
    ///
    /// # Panic
    ///
    /// If the resource exists but is not loaded (i.e. has no content id)
    /// this will panic as this can only happen if there is a bug in the
    /// mail code, or this type was used externally.
    pub fn get(&self, name: &str) -> Option<&ContentId> {
        for possible_source in self.additional_resources {
            if let Some(res) = possible_source.get(name) {
                return Some(
                    res.content_id()
                        .expect("all resources should be loaded/have a content id"),
                );
            }
        }
        return None;
    }
}

impl<'a> Serialize for AdditionalCIds<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut existing_keys = HashSet::new();
        serializer.collect_map(
            self.additional_resources
                .iter()
                .flat_map(|m| {
                    m.iter().map(|(k, resc)| {
                        (
                            k,
                            resc.content_id()
                                .expect("all resources should be loaded/have a content id"),
                        )
                    })
                })
                .filter(|key| existing_keys.insert(key.to_owned())),
        )
    }
}
