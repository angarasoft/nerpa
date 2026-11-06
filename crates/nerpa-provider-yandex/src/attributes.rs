//! The mapping between a declaration's attributes and the wire's shapes: one
//! function per direction per kind, the read-back shared with the next run's
//! read, and the helpers a declaration is read through.

use std::collections::BTreeMap;

use nerpa_core::address::Address;
use nerpa_core::provider::{Attributes, ProviderError};
use nerpa_core::value::Value;

use crate::wire::{
    CreateBucket, Versioning, WireAnonymous, WireBucket, WireExpiration, WireInstance,
    WireNoncurrentExpiration, WireNoncurrentTransition, WireResources, WireRule, WireRuleFilter,
    WireSubnet, WireTag, WireTransition, WireWebsite,
};

/// One gibibyte: Yandex Cloud counts memory in bytes, a person counts in GiB.
const GIB: i64 = 1024 * 1024 * 1024;

/// The attributes a wire instance reads back as.
pub(crate) fn attributes(instance: &WireInstance) -> Attributes {
    let mut attributes = Attributes::new();
    attributes.insert("zone_id".to_owned(), Value::Text(instance.zone_id.clone()));
    attributes.insert(
        "platform_id".to_owned(),
        Value::Text(instance.platform_id.clone()),
    );
    attributes.insert("cores".to_owned(), Value::Integer(instance.resources.cores));
    attributes.insert(
        "memory_gb".to_owned(),
        Value::Integer(instance.resources.memory / GIB),
    );
    if let Some(interface) = instance.network_interfaces.first() {
        attributes.insert(
            "subnet_id".to_owned(),
            Value::Text(interface.subnet_id.clone()),
        );
    }
    attributes
}

/// A required text attribute of a declaration.
pub(crate) fn required_text(
    desired: &Attributes,
    name: &str,
    address: &Address,
) -> Result<String, ProviderError> {
    match desired.get(name) {
        Some(Value::Text(text)) => Ok(text.clone()),
        _ => Err(ProviderError::Declaration {
            address: address.clone(),
            problem: format!("{name} is required and must be text"),
        }),
    }
}

/// A required integer attribute of a declaration.
pub(crate) fn required_integer(
    desired: &Attributes,
    name: &str,
    address: &Address,
) -> Result<i64, ProviderError> {
    match desired.get(name) {
        Some(Value::Integer(value)) => Ok(*value),
        _ => Err(ProviderError::Declaration {
            address: address.clone(),
            problem: format!("{name} is required and must be an integer"),
        }),
    }
}

/// The resources a declaration asks for, in the wire's unit.
pub(crate) fn resources(
    desired: &Attributes,
    address: &Address,
) -> Result<WireResources, ProviderError> {
    Ok(WireResources {
        cores: required_integer(desired, "cores", address)?,
        memory: required_integer(desired, "memory_gb", address)?
            .checked_mul(GIB)
            .ok_or_else(|| ProviderError::Declaration {
                address: address.clone(),
                problem: "memory_gb overflows".to_owned(),
            })?,
    })
}

/// The attributes a subnet reads back as.
pub(crate) fn subnet_attributes(subnet: &WireSubnet) -> Attributes {
    let mut attributes = Attributes::new();
    attributes.insert(
        "network_id".to_owned(),
        Value::Text(subnet.network_id.clone()),
    );
    attributes.insert("zone_id".to_owned(), Value::Text(subnet.zone_id.clone()));
    attributes.insert(
        "v4_cidr_blocks".to_owned(),
        Value::List(
            subnet
                .v4_cidr_blocks
                .iter()
                .cloned()
                .map(Value::Text)
                .collect(),
        ),
    );
    attributes
}

/// The attributes a wire bucket reads back as. Versioning is always reported,
/// and a bucket with none is reported as disabled: the API omits the zero
/// value, and a declaration of `"disabled"` would never converge against an
/// absence. Tags are always reported as a map, empty where there are none, for
/// the same reason: a declared empty map clears them, and a cleared bucket
/// must read back as cleared. An attribute the declaration does not name is
/// reported and never turned into work.
pub(crate) fn bucket_attributes(bucket: &WireBucket) -> Attributes {
    let mut attributes = Attributes::new();
    if let Some(class) = &bucket.default_storage_class {
        attributes.insert(
            "default_storage_class".to_owned(),
            Value::Text(class.clone()),
        );
    }
    let versioning = bucket.versioning.as_deref().map_or_else(
        || "disabled".to_owned(),
        |word| {
            Versioning::of_management(word)
                .map_or_else(|| word, |versioning| versioning.declared())
                .to_owned()
        },
    );
    attributes.insert("versioning".to_owned(), Value::Text(versioning));
    if let Some(size) = bucket.max_size {
        attributes.insert("max_size".to_owned(), Value::Integer(size));
    }
    attributes.insert("tags".to_owned(), Value::Map(tag_map(&bucket.tags)));
    if let Some(website) = &bucket.website {
        let mut pages = BTreeMap::new();
        if let Some(index) = &website.index {
            pages.insert("index".to_owned(), Value::Text(index.clone()));
        }
        if let Some(error) = &website.error {
            pages.insert("error".to_owned(), Value::Text(error.clone()));
        }
        attributes.insert("website".to_owned(), Value::Map(pages));
    }
    // The flags that are on, and only those: the map is the set of the on
    // flags, the shape the declaration speaks and the wire itself omits the
    // off ones in.
    // The rules are always reported, an empty list where there are none:
    // the empty list is the declared word for no rules, and a read that
    // omitted it would never confirm the clear (the tags' own reason).
    attributes.insert(
        "lifecycle".to_owned(),
        Value::List(bucket.lifecycle.iter().map(rule_map).collect()),
    );
    if let Some(anonymous) = bucket.anonymous {
        let mut flags = BTreeMap::new();
        if anonymous.read {
            flags.insert("read".to_owned(), Value::Boolean(true));
        }
        if anonymous.list {
            flags.insert("list".to_owned(), Value::Boolean(true));
        }
        if anonymous.config_read {
            flags.insert("config_read".to_owned(), Value::Boolean(true));
        }
        attributes.insert("anonymous_access".to_owned(), Value::Map(flags));
    }
    attributes
}

/// The tags a wire bucket holds, as a value map.
pub(crate) fn tag_map(tags: &[WireTag]) -> BTreeMap<String, Value> {
    tags.iter()
        .map(|tag| (tag.key.clone(), Value::Text(tag.value.clone())))
        .collect()
}

/// A required list-of-text attribute of a declaration.
pub(crate) fn required_list(
    desired: &Attributes,
    name: &str,
    address: &Address,
) -> Result<Vec<String>, ProviderError> {
    match desired.get(name) {
        Some(Value::List(values)) => values
            .iter()
            .map(|value| match value {
                Value::Text(text) => Ok(text.clone()),
                _ => Err(ProviderError::Declaration {
                    address: address.clone(),
                    problem: format!("{name} must be a list of text"),
                }),
            })
            .collect(),
        _ => Err(ProviderError::Declaration {
            address: address.clone(),
            problem: format!("{name} is required and must be a list"),
        }),
    }
}

/// A text attribute a declaration may manage, or `None` where it does not.
pub(crate) fn optional_text(
    desired: &Attributes,
    name: &str,
    address: &Address,
) -> Result<Option<String>, ProviderError> {
    match desired.get(name) {
        None => Ok(None),
        Some(Value::Text(text)) => Ok(Some(text.clone())),
        Some(_) => Err(ProviderError::Declaration {
            address: address.clone(),
            problem: format!("{name} must be text where it is declared"),
        }),
    }
}

/// An integer attribute a declaration may manage, or `None` where it does not.
pub(crate) fn optional_integer(
    desired: &Attributes,
    name: &str,
    address: &Address,
) -> Result<Option<i64>, ProviderError> {
    match desired.get(name) {
        None => Ok(None),
        Some(Value::Integer(value)) => Ok(Some(*value)),
        Some(_) => Err(ProviderError::Declaration {
            address: address.clone(),
            problem: format!("{name} must be an integer where it is declared"),
        }),
    }
}

/// The tags a declaration manages, in the API's spelling, or an empty list
/// where the declaration leaves them alone. An empty declared map clears the
/// tags, so it is carried as an empty list rather than as an absence.
pub(crate) fn wire_tags(
    desired: &Attributes,
    address: &Address,
) -> Result<Vec<WireTag>, ProviderError> {
    match desired.get("tags") {
        None => Ok(Vec::new()),
        Some(Value::Map(entries)) => entries
            .iter()
            .map(|(key, value)| match value {
                Value::Text(text) => Ok(WireTag {
                    key: key.clone(),
                    value: text.clone(),
                }),
                _ => Err(ProviderError::Declaration {
                    address: address.clone(),
                    problem: format!("tags hold text; {key} does not"),
                }),
            })
            .collect(),
        Some(_) => Err(ProviderError::Declaration {
            address: address.clone(),
            problem: "tags must be a map of text where they are declared".to_owned(),
        }),
    }
}

/// The website a declaration manages, in the wire's shape. The index page
/// is required — the wire refuses a document without one — and the error
/// page is the declaration's to name.
///
/// # Errors
///
/// Returns [`ProviderError::Declaration`] where the website is declared
/// without an index page, or with a value that is not a map of pages.
pub(crate) fn declared_website(
    desired: &Attributes,
    address: &Address,
) -> Result<Option<WireWebsite>, ProviderError> {
    match desired.get("website") {
        None => Ok(None),
        Some(Value::Map(pages)) => {
            let index = match pages.get("index") {
                Some(Value::Text(page)) => Some(page.clone()),
                None => None,
                Some(_) => {
                    return Err(ProviderError::Declaration {
                        address: address.clone(),
                        problem: "website pages hold text; index does not".to_owned(),
                    });
                }
            };
            let error = match pages.get("error") {
                Some(Value::Text(page)) => Some(page.clone()),
                None => None,
                Some(_) => {
                    return Err(ProviderError::Declaration {
                        address: address.clone(),
                        problem: "website pages hold text; error does not".to_owned(),
                    });
                }
            };
            if index.as_deref().is_none_or(str::is_empty) {
                return Err(ProviderError::Declaration {
                    address: address.clone(),
                    problem: "a website is declared without an index page; the wire                               refuses a document without one"
                        .to_owned(),
                });
            }
            Ok(Some(WireWebsite { index, error }))
        }
        Some(_) => Err(ProviderError::Declaration {
            address: address.clone(),
            problem: "website must be a map of pages where it is declared".to_owned(),
        }),
    }
}

/// The anonymous access flags a declaration manages, in the wire's shape:
/// the whole document, for a flag the declaration does not name reads as
/// false.
///
/// # Errors
///
/// Returns [`ProviderError::Declaration`] where the flags are declared as
/// something other than a map of booleans.
pub(crate) fn declared_anonymous(
    desired: &Attributes,
    address: &Address,
) -> Result<Option<WireAnonymous>, ProviderError> {
    match desired.get("anonymous_access") {
        None => Ok(None),
        Some(Value::Map(flags)) => {
            // A flag the declaration names is on; an unnamed flag is off
            // already. Naming one off is refused: the map is the set of the
            // flags that are on, and the read answers in the same shape.
            let flag = |name: &str| -> Result<bool, ProviderError> {
                match flags.get(name) {
                    None => Ok(false),
                    Some(Value::Boolean(true)) => Ok(true),
                    Some(Value::Boolean(false)) => Err(ProviderError::Declaration {
                        address: address.clone(),
                        problem: format!(
                            "anonymous_access names the flags that are on; {name} is \
                             named off, and an unnamed flag is off already"
                        ),
                    }),
                    Some(_) => Err(ProviderError::Declaration {
                        address: address.clone(),
                        problem: format!("anonymous_access holds booleans; {name} does not"),
                    }),
                }
            };
            Ok(Some(WireAnonymous {
                read: flag("read")?,
                list: flag("list")?,
                config_read: flag("config_read")?,
            }))
        }
        Some(_) => Err(ProviderError::Declaration {
            address: address.clone(),
            problem: "anonymous_access must be a map of booleans where it is declared".to_owned(),
        }),
    }
}

/// One transition a declaration names: after how many days, to which storage
/// class.
fn declared_transition(
    pages: &BTreeMap<String, Value>,
    address: &Address,
    kind: &str,
) -> Result<WireTransition, ProviderError> {
    let days = match pages.get("days") {
        Some(Value::Integer(days)) => Some(*days),
        None => None,
        Some(_) => {
            return Err(ProviderError::Declaration {
                address: address.clone(),
                problem: format!("{kind} days must be an integer where it is named"),
            });
        }
    };
    let storage_class = match pages.get("storage_class") {
        Some(Value::Text(class)) => class.clone(),
        _ => {
            return Err(ProviderError::Declaration {
                address: address.clone(),
                problem: format!("{kind} needs a storage_class"),
            });
        }
    };
    Ok(WireTransition {
        days,
        storage_class,
    })
}

/// One noncurrent transition a declaration names: the same shape, after the
/// replacing.
fn declared_noncurrent_transition(
    pages: &BTreeMap<String, Value>,
    address: &Address,
    kind: &str,
) -> Result<WireNoncurrentTransition, ProviderError> {
    let days = match pages.get("days") {
        Some(Value::Integer(days)) => Some(*days),
        None => None,
        Some(_) => {
            return Err(ProviderError::Declaration {
                address: address.clone(),
                problem: format!("{kind} days must be an integer where it is named"),
            });
        }
    };
    let storage_class = match pages.get("storage_class") {
        Some(Value::Text(class)) => class.clone(),
        _ => {
            return Err(ProviderError::Declaration {
                address: address.clone(),
                problem: format!("{kind} needs a storage_class"),
            });
        }
    };
    Ok(WireNoncurrentTransition {
        days,
        storage_class,
    })
}

/// The transitions a declaration names, in the wire's shape.
fn declared_transitions(
    rule: &BTreeMap<String, Value>,
    address: &Address,
    name: &str,
) -> Result<Vec<WireTransition>, ProviderError> {
    match rule.get(name) {
        None => Ok(Vec::new()),
        Some(Value::List(pages)) => pages
            .iter()
            .map(|page| match page {
                Value::Map(pages) => declared_transition(pages, address, name),
                _ => Err(ProviderError::Declaration {
                    address: address.clone(),
                    problem: format!("{name} hold maps of days and storage_class"),
                }),
            })
            .collect(),
        Some(_) => Err(ProviderError::Declaration {
            address: address.clone(),
            problem: format!("{name} must be a list where it is declared"),
        }),
    }
}

/// The noncurrent transitions a declaration names, in the wire's shape.
fn declared_noncurrent_transitions(
    rule: &BTreeMap<String, Value>,
    address: &Address,
    name: &str,
) -> Result<Vec<WireNoncurrentTransition>, ProviderError> {
    match rule.get(name) {
        None => Ok(Vec::new()),
        Some(Value::List(pages)) => pages
            .iter()
            .map(|page| match page {
                Value::Map(pages) => declared_noncurrent_transition(pages, address, name),
                _ => Err(ProviderError::Declaration {
                    address: address.clone(),
                    problem: format!("{name} hold maps of days and storage_class"),
                }),
            })
            .collect(),
        Some(_) => Err(ProviderError::Declaration {
            address: address.clone(),
            problem: format!("{name} must be a list where it is declared"),
        }),
    }
}

/// A rule's flat day count, where it names one.
fn declared_days(
    rule: &BTreeMap<String, Value>,
    address: &Address,
    name: &str,
) -> Result<Option<i64>, ProviderError> {
    match rule.get(name) {
        None => Ok(None),
        Some(Value::Integer(days)) => Ok(Some(*days)),
        Some(_) => Err(ProviderError::Declaration {
            address: address.clone(),
            problem: format!("{name} must be an integer where it is declared"),
        }),
    }
}

/// The lifecycle a declaration manages, in the wire's shape. Each rule names
/// its id — the list is compared whole, and a generated id would never
/// converge — its enabled, a prefix, and at least one of the four actions;
/// an empty list is the word for no rules.
///
/// # Errors
///
/// Returns [`ProviderError::Declaration`] where a rule is not a map, lacks
/// its id, names no action, or holds a value of the wrong shape.
pub(crate) fn declared_lifecycle(
    desired: &Attributes,
    address: &Address,
) -> Result<Vec<WireRule>, ProviderError> {
    let Some(Value::List(rules)) = desired.get("lifecycle") else {
        return Ok(Vec::new());
    };
    let mut wire = Vec::new();
    for rule in rules {
        let Value::Map(rule) = rule else {
            return Err(ProviderError::Declaration {
                address: address.clone(),
                problem: "lifecycle holds maps of rules; one does not".to_owned(),
            });
        };
        let id = match rule.get("id") {
            Some(Value::Text(id)) => Some(id.clone()),
            _ => {
                return Err(ProviderError::Declaration {
                    address: address.clone(),
                    problem: "a lifecycle rule needs an id; the list is compared whole".to_owned(),
                });
            }
        };
        let enabled = match rule.get("enabled") {
            None => true,
            Some(Value::Boolean(set)) => *set,
            Some(_) => {
                return Err(ProviderError::Declaration {
                    address: address.clone(),
                    problem: "enabled must be a boolean where it is named".to_owned(),
                });
            }
        };
        let prefix = match rule.get("prefix") {
            None => None,
            Some(Value::Text(prefix)) => Some(prefix.clone()),
            Some(_) => {
                return Err(ProviderError::Declaration {
                    address: address.clone(),
                    problem: "prefix must be text where it is named".to_owned(),
                });
            }
        };
        let transitions = declared_transitions(rule, address, "transitions")?;
        let noncurrent_transitions =
            declared_noncurrent_transitions(rule, address, "noncurrent_transitions")?;
        let expire_after_days = declared_days(rule, address, "expire_after_days")?;
        let noncurrent_expire_after_days =
            declared_days(rule, address, "noncurrent_expire_after_days")?;
        if transitions.is_empty()
            && noncurrent_transitions.is_empty()
            && expire_after_days.is_none()
            && noncurrent_expire_after_days.is_none()
        {
            return Err(ProviderError::Declaration {
                address: address.clone(),
                problem: "a lifecycle rule names no action; it needs a transition or an                           expiration"
                    .to_owned(),
            });
        }
        wire.push(WireRule {
            id,
            enabled,
            filter: prefix.map(|prefix| WireRuleFilter { prefix }),
            expiration: expire_after_days.map(|days| WireExpiration { days: Some(days) }),
            transitions,
            noncurrent_transitions,
            noncurrent_expiration: noncurrent_expire_after_days
                .map(|days| WireNoncurrentExpiration { days: Some(days) }),
        });
    }
    Ok(wire)
}

/// The rule map a declaration speaks, from the wire's rule: the zero
/// subfields the read materializes are folded away, so what is reported is
/// what the declaration says.
fn rule_map(rule: &WireRule) -> Value {
    let mut map = BTreeMap::new();
    if let Some(id) = &rule.id {
        map.insert("id".to_owned(), Value::Text(id.clone()));
    }
    // The enabled word only where it is off: the map reports the
    // non-default, the same shape the declaration speaks — a rule is on
    // unless it says otherwise, and a wire that materializes the default
    // would never converge against a declaration that left it unnamed.
    if !rule.enabled {
        map.insert("enabled".to_owned(), Value::Boolean(false));
    }
    if let Some(prefix) = rule
        .filter
        .as_ref()
        .map(|filter| filter.prefix.clone())
        .filter(|prefix| !prefix.is_empty())
    {
        map.insert("prefix".to_owned(), Value::Text(prefix));
    }
    if !rule.transitions.is_empty() {
        map.insert(
            "transitions".to_owned(),
            Value::List(
                rule.transitions
                    .iter()
                    .map(|transition| {
                        let mut pages = BTreeMap::new();
                        if let Some(days) = transition.days {
                            pages.insert("days".to_owned(), Value::Integer(days));
                        }
                        pages.insert(
                            "storage_class".to_owned(),
                            Value::Text(transition.storage_class.clone()),
                        );
                        Value::Map(pages)
                    })
                    .collect(),
            ),
        );
    }
    if !rule.noncurrent_transitions.is_empty() {
        map.insert(
            "noncurrent_transitions".to_owned(),
            Value::List(
                rule.noncurrent_transitions
                    .iter()
                    .map(|transition| {
                        let mut pages = BTreeMap::new();
                        if let Some(days) = transition.days {
                            pages.insert("days".to_owned(), Value::Integer(days));
                        }
                        pages.insert(
                            "storage_class".to_owned(),
                            Value::Text(transition.storage_class.clone()),
                        );
                        Value::Map(pages)
                    })
                    .collect(),
            ),
        );
    }
    if let Some(days) = rule.expiration.as_ref().and_then(|when| when.days) {
        map.insert("expire_after_days".to_owned(), Value::Integer(days));
    }
    if let Some(days) = rule
        .noncurrent_expiration
        .as_ref()
        .and_then(|when| when.days)
    {
        map.insert(
            "noncurrent_expire_after_days".to_owned(),
            Value::Integer(days),
        );
    }
    Value::Map(map)
}

/// The create request a declared bucket becomes. Only what the declaration
/// manages is carried.
pub(crate) fn create_bucket(
    desired: &Attributes,
    address: &Address,
    folder_id: &str,
) -> Result<CreateBucket, ProviderError> {
    Ok(CreateBucket {
        folder_id: folder_id.to_owned(),
        name: wire_name(address),
        default_storage_class: optional_text(desired, "default_storage_class", address)?,
        versioning: Versioning::declared_of(desired, address)?
            .map(|versioning| versioning.management().to_owned()),
        max_size: optional_integer(desired, "max_size", address)?,
        tags: wire_tags(desired, address)?,
        // The site is not carried: the wire ignores the document at
        // creation — the site joins a bucket through its first update
        // (adr-2026-082). The flags and the rules are carried: the wire
        // takes them there (adr-2026-083).
        anonymous: declared_anonymous(desired, address)?,
        lifecycle: declared_lifecycle(desired, address)?,
    })
}

/// The name of a resource, as the wire knows it.
pub(crate) fn wire_name(address: &Address) -> String {
    address.name().as_str().to_owned()
}
