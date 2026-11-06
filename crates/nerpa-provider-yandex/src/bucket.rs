//! The bucket's two doors: the management one for the attributes it holds,
//! the S3 one for the word that changes (adr-2026-081). The storage API names
//! a bucket by its name, which is also its id.

use nerpa_core::address::Address;
use nerpa_core::provider::{Concluded, Prepared, ProviderError, Sending};

use crate::attributes::{
    bucket_attributes, create_bucket, optional_integer, optional_text, wire_tags,
};
use crate::wire::{
    PatchBucket, Versioning, WireAllowedPrivateEndpoints, WireBucket, WireObjectLock,
};
use crate::{ApiError, Response, Yandex, world};

impl Yandex {
    /// One bucket by name, read in the API's full view: the basic one the
    /// list answers with does not carry the tags. `None` where the bucket is
    /// not there — the API answers 404, which is an answer rather than a
    /// failure.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::World`] where the API could not be asked, or
    /// answered with something other than the bucket or its absence.
    pub(crate) fn get_bucket(
        &self,
        name: &str,
        headers: &[(&str, &str)],
        address: &Address,
    ) -> Result<Option<WireBucket>, ProviderError> {
        let url = format!("{}/buckets/{}?view=VIEW_FULL", self.storage, name);
        match self.api.get(&url, headers) {
            Ok(Response { status: 200, body }) => {
                serde_json::from_str(&body)
                    .map(Some)
                    .map_err(|error| ProviderError::World {
                        address: address.clone(),
                        problem: error.to_string(),
                    })
            }
            Ok(Response { status, .. }) => Err(ProviderError::World {
                address: address.clone(),
                problem: format!("the API answered {status} to a bucket read"),
            }),
            Err(ApiError::Refused { status: 404, .. }) => Ok(None),
            Err(error) => Err(ProviderError::World {
                address: address.clone(),
                problem: error.to_string(),
            }),
        }
    }

    /// Makes a bucket.
    fn make_bucket(
        &self,
        change: &Prepared<'_>,
        headers: &[(&str, &str)],
    ) -> Result<(), ProviderError> {
        let create = create_bucket(change.desired, change.address, &self.folder_id)?;
        let body = serde_json::to_string(&create).map_err(|error| ProviderError::World {
            address: change.address.clone(),
            problem: error.to_string(),
        })?;
        let url = format!("{}/buckets", self.storage);
        world(change.address, self.api.post(&url, headers, &body))?;
        Ok(())
    }

    /// Reads one bucket back as what it became.
    fn read_bucket(
        &self,
        change: &Prepared<'_>,
        headers: &[(&str, &str)],
        name: &str,
    ) -> Result<Concluded, ProviderError> {
        let bucket = self
            .get_bucket(name, headers, change.address)?
            .ok_or_else(|| ProviderError::World {
                address: change.address.clone(),
                problem: "the bucket is not there after it was made".to_owned(),
            })?;
        Ok(Concluded::Became(bucket_attributes(&bucket)))
    }

    /// Removes a bucket by name. The API answers 404 where there is nothing
    /// to remove, which is what the removal wanted anyway.
    fn remove_bucket(
        &self,
        change: &Prepared<'_>,
        headers: &[(&str, &str)],
        name: &str,
    ) -> Result<(), ProviderError> {
        let url = format!("{}/buckets/{}", self.storage, name);
        match self.api.delete(&url, headers) {
            // The API answers 404 where there is nothing to remove, which is
            // what the removal wanted anyway.
            Ok(_) | Err(ApiError::Refused { status: 404, .. }) => Ok(()),
            Err(error) => Err(ProviderError::World {
                address: change.address.clone(),
                problem: error.to_string(),
            }),
        }
    }

    /// Corrects a bucket in place. The management update cannot carry a
    /// versioning but the enabled word its validation demands, so the
    /// attributes it manages are corrected first with that word in the body,
    /// and the S3 door then writes the versioning the declaration asks for —
    /// the final word (adr-2026-081). A declaration that does not manage
    /// versioning, on a bucket whose versioning is not enabled, is refused by
    /// name: the enabled word the management body carries would flip it.
    fn correct_bucket(
        &self,
        change: &Prepared<'_>,
        headers: &[(&str, &str)],
        name: &str,
    ) -> Result<(), ProviderError> {
        let desired = change.desired;
        let manages = ["default_storage_class", "max_size", "tags"]
            .iter()
            .any(|attribute| desired.get(*attribute).is_some());
        if !manages {
            if desired.get("versioning").is_some() {
                return self.write_versioning(change, headers, name);
            }
            return Err(ProviderError::Declaration {
                address: change.address.clone(),
                problem: "an update with nothing the storage API can change".to_owned(),
            });
        }

        let current = self
            .get_bucket(name, headers, change.address)?
            .ok_or_else(|| ProviderError::World {
                address: change.address.clone(),
                problem: "the bucket to update is not there".to_owned(),
            })?;
        let held = current
            .versioning
            .clone()
            .unwrap_or_else(|| "VERSIONING_DISABLED".to_owned());
        if held != "VERSIONING_ENABLED" && desired.get("versioning").is_none() {
            return Err(ProviderError::World {
                address: change.address.clone(),
                problem: format!(
                    "the management update carries the enabled word, which \
                     would flip this bucket's versioning ({held}); declare \
                     versioning to change it, or enable it first"
                ),
            });
        }
        let patch = PatchBucket {
            versioning: "VERSIONING_ENABLED".to_owned(),
            default_storage_class: optional_text(desired, "default_storage_class", change.address)?
                .or_else(|| current.default_storage_class.clone()),
            max_size: optional_integer(desired, "max_size", change.address)?.or(current.max_size),
            tags: match desired.get("tags") {
                Some(_) => wire_tags(desired, change.address)?,
                None => current.tags.clone(),
            },
            // Carried from the read, where the bucket holds them: the wire
            // applies what the body carries, mask or no mask, so the fields
            // the declaration does not name must say what the world already
            // says. Where the read carried none, the shapes the validation
            // demands are stated: the check off with the console forced,
            // the lock disabled.
            allowed_private_endpoints: current.allowed_private_endpoints.clone().unwrap_or(
                WireAllowedPrivateEndpoints {
                    enabled: false,
                    force_cloud_console_access: true,
                },
            ),
            object_lock: current.object_lock.clone().unwrap_or(WireObjectLock {
                status: "OBJECT_LOCK_STATUS_DISABLED".to_owned(),
            }),
        };
        let mut mask = Vec::new();
        if desired.get("default_storage_class").is_some() {
            mask.push("defaultStorageClass");
        }
        if desired.get("max_size").is_some() {
            mask.push("maxSize");
        }
        if desired.get("tags").is_some() {
            mask.push("tags");
        }
        let body = serde_json::to_string(&patch).map_err(|error| ProviderError::World {
            address: change.address.clone(),
            problem: error.to_string(),
        })?;
        let url = format!(
            "{}/buckets/{}?updateMask={}",
            self.storage,
            name,
            mask.join(",")
        );
        world(change.address, self.api.patch(&url, headers, &body))?;
        if desired.get("versioning").is_some() {
            return self.write_versioning(change, headers, name);
        }
        Ok(())
    }

    /// Writes a bucket's versioning through the S3 door: the one document
    /// that door speaks, with the word the declaration asks for
    /// (adr-2026-081). The token the door takes is the same short-lived one
    /// the management door takes, as a bearer and with no signature.
    fn write_versioning(
        &self,
        change: &Prepared<'_>,
        headers: &[(&str, &str)],
        name: &str,
    ) -> Result<(), ProviderError> {
        let Some(versioning) = Versioning::declared_of(change.desired, change.address)? else {
            return Err(ProviderError::Declaration {
                address: change.address.clone(),
                problem: "a versioning update with no versioning declared".to_owned(),
            });
        };
        let body = format!(
            "<VersioningConfiguration><Status>{}</Status></VersioningConfiguration>",
            versioning.s3()
        );
        let url = format!("{}/{name}?versioning", self.s3);
        let mut xml = headers.to_vec();
        xml.push(("Content-Type", "application/xml"));
        world(change.address, self.api.put(&url, &xml, &body)).map(|_| ())
    }

    /// Applies a bucket change, and reports what it became. The storage API
    /// names a bucket by its name, which is also its id.
    pub(crate) fn act_bucket(
        &self,
        change: &Prepared<'_>,
        headers: &[(&str, &str)],
    ) -> Result<Concluded, ProviderError> {
        let name = held_bucket_name(change.address)?;
        let name = name.as_str();
        match change.kind {
            Sending::Delete => {
                self.remove_bucket(change, headers, name)?;
                Ok(Concluded::Gone)
            }
            Sending::Replace => {
                self.remove_bucket(change, headers, name)?;
                self.make_bucket(change, headers)?;
                self.read_bucket(change, headers, name)
            }
            Sending::Update => {
                self.correct_bucket(change, headers, name)?;
                self.read_bucket(change, headers, name)
            }
            Sending::Create => {
                self.make_bucket(change, headers)?;
                self.read_bucket(change, headers, name)
            }
            _ => Err(ProviderError::Declaration {
                address: change.address.clone(),
                problem: "an unknown change kind reached a bucket".to_owned(),
            }),
        }
    }
}

/// The name a bucket's doors are addressed by: one the cloud can hold. The
/// address grammar is wider than the cloud — a foreign name may carry a
/// slash, which portage paths asked for — and a bucket's name goes into the
/// URL path as it stands, so the wall is drawn here: a name the storage
/// cannot hold is refused by name rather than addressing another resource's
/// path (w-yc-bucket-name).
///
/// # Errors
///
/// Returns [`ProviderError::Declaration`] where the name is not one the
/// storage holds: three to sixty-three characters, lower case, digits and
/// hyphens, a letter or digit at each end.
pub(crate) fn held_bucket_name(address: &Address) -> Result<String, ProviderError> {
    let name = address.name().as_str();
    let shape = (3..=63).contains(&name.len())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && name
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && name
            .bytes()
            .last()
            .is_some_and(|byte| byte.is_ascii_alphanumeric());
    if shape {
        Ok(name.to_owned())
    } else {
        Err(ProviderError::Declaration {
            address: address.clone(),
            problem: format!(
                "{name:?} is not a name the storage holds: three to sixty-three \
                 characters, lower case, digits and hyphens, a letter or digit at each end"
            ),
        })
    }
}
