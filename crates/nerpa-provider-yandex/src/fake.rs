//! The in-memory fake: an API that keeps what it is given. Public through
//! [`crate::Yandex::fake`] — the `Provider::fake` constructor is a trait
//! method, and integration tests in other crates cannot see anything less
//! than the crate, so the fake compiles into the artifact; nothing else
//! reaches for it.

use std::collections::BTreeMap;

use crate::api::{Api, ApiError, Response};
use crate::wire::{
    CreateBucket, CreateInstance, CreateNetwork, CreateSubnet, PatchBucket, PatchInstance,
    WireBucket, WireInstance, WireNetwork, WireSubnet,
};

/// An API that keeps instances, networks, subnets and buckets in memory, for
/// [`Yandex::fake`] and the tests.
#[derive(Debug, Default)]
pub(crate) struct Memory {
    instances: std::sync::Mutex<BTreeMap<String, WireInstance>>,
    networks: std::sync::Mutex<BTreeMap<String, WireNetwork>>,
    subnets: std::sync::Mutex<BTreeMap<String, WireSubnet>>,
    buckets: std::sync::Mutex<BTreeMap<String, WireBucket>>,
}

/// The name a single-bucket URL addresses, where it addresses one.
fn bucket_name(url: &str) -> Option<&str> {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    path.rsplit_once("/buckets/").map(|(_, name)| name)
}

/// The resource kind a URL addresses: `instances`, `networks` or `subnets`.
fn list_kind(url: &str) -> &str {
    if url.contains("/networks") {
        "networks"
    } else if url.contains("/subnets") {
        "subnets"
    } else {
        "instances"
    }
}

/// What the API answers where a bucket is not there.
fn not_there() -> String {
    r#"{"code":5,"message":"The specified bucket does not exist."}"#.to_owned()
}

/// A poisoned store, as the transport error it is.
fn poisoned() -> ApiError {
    ApiError::Transport("the memory store is poisoned".to_owned())
}

impl Api for Memory {
    fn get(&self, url: &str, _headers: &[(&str, &str)]) -> Result<Response, ApiError> {
        // One bucket by name, in the full view the provider asks for, with
        // the 404 the real API answers where it is not there.
        if let Some(name) = bucket_name(url) {
            let buckets = self.buckets.lock().map_err(|_| poisoned())?;
            return match buckets.get(name) {
                Some(bucket) => {
                    let body = serde_json::to_string(bucket)
                        .map_err(|error| ApiError::Transport(error.to_string()))?;
                    Ok(Response { status: 200, body })
                }
                None => Response {
                    status: 404,
                    body: not_there(),
                }
                .or_refused(),
            };
        }
        let body = match list_kind(url) {
            "networks" => {
                let networks = self.networks.lock().map_err(|_| poisoned())?;
                let mut listed: Vec<&WireNetwork> = networks.values().collect();
                listed.sort_by(|left, right| left.name.cmp(&right.name));
                serde_json::to_string(&serde_json::json!({ "networks": listed }))
            }
            "subnets" => {
                let subnets = self.subnets.lock().map_err(|_| poisoned())?;
                let mut listed: Vec<&WireSubnet> = subnets.values().collect();
                listed.sort_by(|left, right| left.name.cmp(&right.name));
                serde_json::to_string(&serde_json::json!({ "subnets": listed }))
            }
            _ => {
                let instances = self.instances.lock().map_err(|_| poisoned())?;
                let mut listed: Vec<&WireInstance> = instances.values().collect();
                listed.sort_by(|left, right| left.name.cmp(&right.name));
                serde_json::to_string(&serde_json::json!({ "instances": listed }))
            }
        }
        .map_err(|error| ApiError::Transport(error.to_string()))?;
        Ok(Response { status: 200, body })
    }

    fn post(&self, url: &str, _headers: &[(&str, &str)], body: &str) -> Result<Response, ApiError> {
        if url.contains("/buckets") {
            let create: CreateBucket = serde_json::from_str(body)
                .map_err(|error| ApiError::Transport(error.to_string()))?;
            self.buckets.lock().map_err(|_| poisoned())?.insert(
                create.name.clone(),
                WireBucket {
                    name: create.name,
                    default_storage_class: create.default_storage_class,
                    versioning: create.versioning,
                    max_size: create.max_size,
                    tags: create.tags,
                    object_lock: None,
                    allowed_private_endpoints: None,
                },
            );
            return Ok(Response {
                status: 200,
                body: "{}".to_owned(),
            });
        }
        match list_kind(url) {
            "networks" => {
                let create: CreateNetwork = serde_json::from_str(body)
                    .map_err(|error| ApiError::Transport(error.to_string()))?;
                self.networks.lock().map_err(|_| poisoned())?.insert(
                    create.name.clone(),
                    WireNetwork {
                        id: format!("id-{}", create.name),
                        name: create.name,
                    },
                );
            }
            "subnets" => {
                let create: CreateSubnet = serde_json::from_str(body)
                    .map_err(|error| ApiError::Transport(error.to_string()))?;
                self.subnets.lock().map_err(|_| poisoned())?.insert(
                    create.name.clone(),
                    WireSubnet {
                        id: format!("id-{}", create.name),
                        name: create.name,
                        network_id: create.network_id,
                        zone_id: create.zone_id,
                        v4_cidr_blocks: create.v4_cidr_blocks,
                    },
                );
            }
            _ => {
                let create: CreateInstance = serde_json::from_str(body)
                    .map_err(|error| ApiError::Transport(error.to_string()))?;
                self.instances.lock().map_err(|_| poisoned())?.insert(
                    create.name.clone(),
                    WireInstance {
                        id: format!("id-{}", create.name),
                        name: create.name,
                        zone_id: create.zone_id,
                        platform_id: create.platform_id,
                        resources: create.resources_spec,
                        network_interfaces: Vec::new(),
                    },
                );
            }
        }
        Ok(Response {
            status: 200,
            body: "{}".to_owned(),
        })
    }

    fn patch(
        &self,
        url: &str,
        _headers: &[(&str, &str)],
        body: &str,
    ) -> Result<Response, ApiError> {
        // A bucket is corrected by what the body carries — the mask is
        // decorative on the real wire — and an instance through its
        // resources. Networks and subnets replace rather than patch.
        if let Some(name) = bucket_name(url) {
            let patch: PatchBucket = serde_json::from_str(body)
                .map_err(|error| ApiError::Transport(error.to_string()))?;
            let mut buckets = self.buckets.lock().map_err(|_| poisoned())?;
            // The real API answers 404 where the bucket is not there; a fake
            // that answered 200 would keep a test green that misnamed its
            // own resource.
            let Some(bucket) = buckets.get_mut(name) else {
                return Response {
                    status: 404,
                    body: not_there(),
                }
                .or_refused();
            };
            bucket.default_storage_class = patch.default_storage_class;
            bucket.versioning = Some(patch.versioning);
            bucket.max_size = patch.max_size;
            bucket.tags = patch.tags;
            return Ok(Response {
                status: 200,
                body: "{}".to_owned(),
            });
        }
        let patch: PatchInstance =
            serde_json::from_str(body).map_err(|error| ApiError::Transport(error.to_string()))?;
        let id = url.rsplit('/').next().unwrap_or("");
        let mut instances = self.instances.lock().map_err(|_| poisoned())?;
        if let Some(instance) = instances.values_mut().find(|instance| instance.id == id) {
            instance.resources = patch.resources_spec;
        }
        Ok(Response {
            status: 200,
            body: "{}".to_owned(),
        })
    }

    fn put(&self, url: &str, _headers: &[(&str, &str)], body: &str) -> Result<Response, ApiError> {
        // The S3 door writes the versioning document: the one sub-resource
        // this fake answers for, the one the real door speaks.
        if !url.contains("?versioning") {
            return Err(ApiError::Transport(format!(
                "this fake does not speak PUT for {url}"
            )));
        }
        let path = url.split('?').next().unwrap_or("");
        let name = path.rsplit('/').next().unwrap_or("");
        let enabled = body.contains("<Status>Enabled</Status>");
        let suspended = body.contains("<Status>Suspended</Status>");
        if !enabled && !suspended {
            return Err(ApiError::Transport(format!(
                "this fake does not read the versioning document: {body}"
            )));
        }
        let mut buckets = self.buckets.lock().map_err(|_| poisoned())?;
        let Some(bucket) = buckets.get_mut(name) else {
            return Response {
                status: 404,
                body: not_there(),
            }
            .or_refused();
        };
        bucket.versioning = Some(if enabled {
            "VERSIONING_ENABLED".to_owned()
        } else {
            "VERSIONING_SUSPENDED".to_owned()
        });
        Ok(Response {
            status: 200,
            body: String::new(),
        })
    }

    fn delete(&self, url: &str, _headers: &[(&str, &str)]) -> Result<Response, ApiError> {
        // A bucket is removed by name, with the 404 the real API answers
        // where there was nothing to remove.
        if let Some(name) = bucket_name(url) {
            let removed = self
                .buckets
                .lock()
                .map_err(|_| poisoned())?
                .remove(name)
                .is_some();
            return if removed {
                Ok(Response {
                    status: 200,
                    body: "{}".to_owned(),
                })
            } else {
                Response {
                    status: 404,
                    body: not_there(),
                }
                .or_refused()
            };
        }
        let id = url.rsplit('/').next().unwrap_or("");
        match list_kind(url) {
            "networks" => {
                self.networks
                    .lock()
                    .map_err(|_| poisoned())?
                    .retain(|_, network| network.id != id);
            }
            "subnets" => {
                self.subnets
                    .lock()
                    .map_err(|_| poisoned())?
                    .retain(|_, subnet| subnet.id != id);
            }
            _ => {
                self.instances
                    .lock()
                    .map_err(|_| poisoned())?
                    .retain(|_, instance| instance.id != id);
            }
        }
        Ok(Response {
            status: 200,
            body: "{}".to_owned(),
        })
    }
}
