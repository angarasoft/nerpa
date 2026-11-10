//! The in-memory fake: an API that keeps what it is given. Public through
//! [`crate::Yandex::fake`] — the `Provider::fake` constructor is a trait
//! method, and integration tests in other crates cannot see anything less
//! than the crate, so the fake compiles into the artifact; nothing else
//! reaches for it.

use std::collections::BTreeMap;

use crate::api::{Api, ApiError, Response};
use crate::wire::{
    CreateBucket, CreateInstance, CreateNetwork, CreateSubnet, PatchBucket, PatchDisk,
    PatchInstance, WireAttachedDisk, WireBucket, WireDisk, WireInstance, WireInterface,
    WireNetwork, WireOperation, WireResources, WireSubnet,
};

/// The size a boot disk is born with where the create carries no size of
/// its own: the image's, as far as this fake is concerned.
const BORN_DISK: i64 = 10 * 1024 * 1024 * 1024;

/// An API that keeps instances, networks, subnets and buckets in memory, for
/// [`Yandex::fake`] and the tests.
#[derive(Debug, Default)]
pub(crate) struct Memory {
    instances: std::sync::Mutex<BTreeMap<String, WireInstance>>,
    networks: std::sync::Mutex<BTreeMap<String, WireNetwork>>,
    subnets: std::sync::Mutex<BTreeMap<String, WireSubnet>>,
    buckets: std::sync::Mutex<BTreeMap<String, WireBucket>>,
    disks: std::sync::Mutex<BTreeMap<String, WireDisk>>,
    operations: std::sync::Mutex<BTreeMap<String, Pending>>,
    next: std::sync::Mutex<u64>,
}

/// An operation the fake holds: how many times it has been asked, whether it
/// is done, and what happens when it is. It finishes on its second ask, so a
/// run proves the provider asks without waiting as long as the real cloud
/// takes.
#[derive(Debug)]
struct Pending {
    asks: u8,
    done: bool,
    effect: Option<Effect>,
}

/// What finishing an operation does to the world the fake holds.
#[derive(Debug)]
enum Effect {
    /// An instance appears, with its boot disk, made as the create asked.
    MakeInstance(Box<(WireInstance, WireDisk)>),
    /// The instance stops.
    Stop(String),
    /// The instance starts.
    Start(String),
    /// The instance's resources change.
    PatchInstance {
        id: String,
        resources: WireResources,
    },
    /// The disk grows.
    GrowDisk { id: String, size: i64 },
    /// The instance goes, and the disk that dies with it goes too.
    RemoveInstance(String),
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

/// The resource a URL addresses: the last step of its path, without its
/// query.
fn resource_id(url: &str) -> &str {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    path.rsplit('/').next().unwrap_or(path)
}

/// The disk a URL addresses, where it addresses one.
fn disk_id(url: &str) -> Option<&str> {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    path.split_once("/disks/").map(|(_, id)| id)
}

/// The operation a URL asks about, where it asks about one.
fn operation_id(url: &str) -> Option<&str> {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    path.split_once("/operations/").map(|(_, id)| id)
}

/// The instance action a URL names, where it names one: the instance it
/// reaches, and the action itself.
fn instance_action(url: &str) -> Option<(String, &'static str)> {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let (id, action) = if let Some(id) = path.strip_suffix(":stop") {
        (id, "stop")
    } else {
        (path.strip_suffix(":start")?, "start")
    };
    Some((id.rsplit('/').next().unwrap_or(id).to_owned(), action))
}

/// What the API answers where a bucket is not there.
fn not_there() -> String {
    r#"{"code":5,"message":"The specified bucket does not exist."}"#.to_owned()
}

/// A poisoned store, as the transport error it is.
fn poisoned() -> ApiError {
    ApiError::Transport("the memory store is poisoned".to_owned())
}

/// One refusal the fake makes, as the API error the real wire would give.
fn refused(status: u16, code: u16, message: &str) -> Result<Response, ApiError> {
    Response {
        status,
        body: format!(r#"{{"code":{code},"message":"{message}"}}"#),
    }
    .or_refused()
}

impl Memory {
    /// An operation begins: the work is taken on, and answered with an
    /// operation that is not yet done.
    fn begins(&self, effect: Effect) -> Result<Response, ApiError> {
        let id = {
            let mut next = self.next.lock().map_err(|_| poisoned())?;
            *next += 1;
            format!("op-{next}")
        };
        self.operations.lock().map_err(|_| poisoned())?.insert(
            id.clone(),
            Pending {
                asks: 0,
                done: false,
                effect: Some(effect),
            },
        );
        let body = serde_json::to_string(&WireOperation {
            id,
            done: false,
            error: None,
        })
        .map_err(|error| ApiError::Transport(error.to_string()))?;
        Ok(Response { status: 200, body })
    }

    /// One ask of an operation: it finishes on the second ask, and what it
    /// holds happens then. A further ask is answered as done, for the asking
    /// is the caller's to stop.
    fn asked_about(&self, id: &str) -> Result<Response, ApiError> {
        let (done, effect) = {
            let mut operations = self.operations.lock().map_err(|_| poisoned())?;
            let Some(pending) = operations.get_mut(id) else {
                return refused(404, 5, "the operation is not there");
            };
            pending.asks += 1;
            let finish = pending.asks >= 2 && !pending.done;
            if finish {
                pending.done = true;
            }
            let effect = if finish { pending.effect.take() } else { None };
            (pending.done, effect)
        };
        if let Some(effect) = effect {
            self.happened(effect)?;
        }
        let body = serde_json::to_string(&WireOperation {
            id: id.to_owned(),
            done,
            error: None,
        })
        .map_err(|error| ApiError::Transport(error.to_string()))?;
        Ok(Response { status: 200, body })
    }

    /// What a finished operation did to the world.
    fn happened(&self, effect: Effect) -> Result<(), ApiError> {
        match effect {
            Effect::MakeInstance(made) => {
                let (instance, disk) = *made;
                self.disks
                    .lock()
                    .map_err(|_| poisoned())?
                    .insert(disk.id.clone(), disk);
                self.instances
                    .lock()
                    .map_err(|_| poisoned())?
                    .insert(instance.name.clone(), instance);
            }
            Effect::Stop(id) => {
                let mut instances = self.instances.lock().map_err(|_| poisoned())?;
                if let Some(instance) = instances.values_mut().find(|i| i.id == id) {
                    instance.status = Some("STOPPED".to_owned());
                }
            }
            Effect::Start(id) => {
                let mut instances = self.instances.lock().map_err(|_| poisoned())?;
                if let Some(instance) = instances.values_mut().find(|i| i.id == id) {
                    instance.status = Some("RUNNING".to_owned());
                }
            }
            Effect::PatchInstance { id, resources } => {
                let mut instances = self.instances.lock().map_err(|_| poisoned())?;
                if let Some(instance) = instances.values_mut().find(|i| i.id == id) {
                    instance.resources = resources;
                }
            }
            Effect::GrowDisk { id, size } => {
                let mut disks = self.disks.lock().map_err(|_| poisoned())?;
                if let Some(disk) = disks.get_mut(&id) {
                    disk.size = size;
                }
            }
            Effect::RemoveInstance(id) => {
                let disk = {
                    let mut instances = self.instances.lock().map_err(|_| poisoned())?;
                    let disk = instances
                        .values()
                        .find(|instance| instance.id == id)
                        .and_then(|instance| instance.boot_disk.as_ref())
                        .map(|boot| boot.disk_id.clone());
                    instances.retain(|_, instance| instance.id != id);
                    disk
                };
                // The boot disk dies with the instance: the create said it
                // would, and a replace that left it behind would leave a
                // disk nobody declared.
                if let Some(disk) = disk {
                    self.disks.lock().map_err(|_| poisoned())?.remove(&disk);
                }
            }
        }
        Ok(())
    }

    /// A stop or a start reaches the instance the URL names, and is work the
    /// cloud takes on: the state flips when the operation is done.
    fn act_on(&self, id: &str, action: &str) -> Result<Response, ApiError> {
        let status = {
            let instances = self.instances.lock().map_err(|_| poisoned())?;
            instances
                .values()
                .find(|instance| instance.id == id)
                .and_then(|instance| instance.status.clone())
        };
        let reaches = matches!(
            (action, status.as_deref()),
            ("stop", Some("RUNNING")) | ("start", Some("STOPPED"))
        );
        if !reaches {
            return refused(
                409,
                9,
                &format!("the instance {id} is not in a state {action} reaches"),
            );
        }
        let effect = if action == "stop" {
            Effect::Stop(id.to_owned())
        } else {
            Effect::Start(id.to_owned())
        };
        self.begins(effect)
    }

    /// An instance is made as the create asked, appearing when the operation
    /// that answers the create is done — as the real cloud makes it.
    fn make_instance(&self, body: &str) -> Result<Response, ApiError> {
        let create: CreateInstance =
            serde_json::from_str(body).map_err(|error| ApiError::Transport(error.to_string()))?;
        let name = create.name;
        let image = create.boot_disk_spec.disk_spec.image_id;
        let size = create.boot_disk_spec.disk_spec.size;
        let instance = WireInstance {
            id: format!("id-{name}"),
            name: name.clone(),
            zone_id: create.zone_id,
            platform_id: create.platform_id,
            resources: create.resources_spec,
            network_interfaces: create
                .network_interface_specs
                .into_iter()
                .map(|spec| WireInterface {
                    subnet_id: spec.subnet_id,
                })
                .collect(),
            status: Some("RUNNING".to_owned()),
            boot_disk: Some(WireAttachedDisk {
                disk_id: format!("disk-{name}"),
            }),
        };
        let disk = WireDisk {
            id: format!("disk-{name}"),
            size: size.unwrap_or(BORN_DISK),
            source_image_id: Some(image),
        };
        self.begins(Effect::MakeInstance(Box::new((instance, disk))))
    }
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
        // One disk by id: what the boot disk holds.
        if let Some(id) = disk_id(url) {
            let disks = self.disks.lock().map_err(|_| poisoned())?;
            return match disks.get(id) {
                Some(disk) => {
                    let body = serde_json::to_string(disk)
                        .map_err(|error| ApiError::Transport(error.to_string()))?;
                    Ok(Response { status: 200, body })
                }
                None => refused(404, 5, "the disk is not there"),
            };
        }
        // One operation by id, asked until it is done.
        if let Some(id) = operation_id(url) {
            return self.asked_about(id);
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
                    website: None,
                    anonymous: create.anonymous,
                    lifecycle: create.lifecycle,
                },
            );
            return Ok(Response {
                status: 200,
                body: "{}".to_owned(),
            });
        }
        // A stop or a start reaches the instance the URL names.
        if let Some((id, action)) = instance_action(url) {
            return self.act_on(&id, action);
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
                return self.make_instance(body);
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
        // resources, a disk through its size. Networks and subnets replace
        // rather than patch.
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
            bucket.website = patch.website;
            // The wire leaves the flags alone when the body omits them: the
            // probe showed an update without the field keeping them.
            if let Some(anonymous) = patch.anonymous {
                bucket.anonymous = Some(anonymous);
            }
            bucket.lifecycle = patch.lifecycle;
            return Ok(Response {
                status: 200,
                body: "{}".to_owned(),
            });
        }
        if let Some(id) = disk_id(url) {
            let patch: PatchDisk = serde_json::from_str(body)
                .map_err(|error| ApiError::Transport(error.to_string()))?;
            let held = {
                let disks = self.disks.lock().map_err(|_| poisoned())?;
                disks.get(id).map(|disk| disk.size)
            };
            let Some(held) = held else {
                return refused(404, 5, "the disk is not there");
            };
            // A disk only grows: the real API refuses a shrink, and the
            // planner refuses one earlier still.
            if patch.size < held {
                return refused(
                    400,
                    3,
                    &format!(
                        "a disk only grows: {held} is there and {} is asked",
                        patch.size
                    ),
                );
            }
            return self.begins(Effect::GrowDisk {
                id: id.to_owned(),
                size: patch.size,
            });
        }
        let patch: PatchInstance =
            serde_json::from_str(body).map_err(|error| ApiError::Transport(error.to_string()))?;
        let id = resource_id(url);
        let running = {
            let instances = self.instances.lock().map_err(|_| poisoned())?;
            instances
                .values()
                .find(|instance| instance.id == id)
                .map(|instance| instance.status.as_deref() == Some("RUNNING"))
        };
        let Some(running) = running else {
            return refused(404, 5, "the instance is not there");
        };
        // The real API refuses a resources change on a running instance: a
        // provider that forgets the stop fails here rather than passing.
        if running {
            return refused(
                400,
                9,
                &format!(
                    "the instance {id} is running; its resources change only while it is stopped"
                ),
            );
        }
        self.begins(Effect::PatchInstance {
            id: id.to_owned(),
            resources: patch.resources_spec,
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
        // An instance's removal is work the cloud takes on: the operation it
        // answers with is awaited, and the instance — with the disk that
        // dies with it — goes when it is done.
        if list_kind(url) == "instances" {
            let id = resource_id(url);
            let there = {
                let instances = self.instances.lock().map_err(|_| poisoned())?;
                instances.values().any(|instance| instance.id == id)
            };
            if !there {
                return refused(404, 5, "the instance is not there");
            }
            return self.begins(Effect::RemoveInstance(id.to_owned()));
        }
        let id = url.rsplit('/').next().unwrap_or("");
        if url.contains("/networks") {
            self.networks
                .lock()
                .map_err(|_| poisoned())?
                .retain(|_, network| network.id != id);
        } else if url.contains("/subnets") {
            self.subnets
                .lock()
                .map_err(|_| poisoned())?
                .retain(|_, subnet| subnet.id != id);
        } else {
            return Err(ApiError::Transport(format!(
                "this fake does not speak DELETE for {url}"
            )));
        }
        Ok(Response {
            status: 200,
            body: "{}".to_owned(),
        })
    }
}
