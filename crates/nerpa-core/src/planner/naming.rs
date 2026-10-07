//! What a resource names on its machine, and what that lets the planner refuse
//! or take away.

use std::collections::BTreeMap;

use crate::address::{Address, Target};
use crate::attribute::Attribute;
use crate::graph::Graph;
use crate::resource::Resource;
use crate::schema::Schema;
use crate::state::{Record, State, Written};
use crate::value::Value;

use super::PlanError;

/// Refuses two resources on one machine that name the same thing there.
///
/// Addresses are unique by construction; what they declare is not. Two files at
/// one path were both created, the last write won, and every later plan proposed
/// the other again. Checked on declared values, so a name that arrives through a
/// reference is known only later and is not caught here.
pub(super) fn one_thing_once(graph: &Graph, schema: &Schema) -> Result<(), PlanError> {
    let mut seen: BTreeMap<(Target, String, String), Address> = BTreeMap::new();
    for stage in graph.stages()? {
        for address in stage.addresses() {
            let (Some(node), Some(resource), Some((attribute, namespace))) = (
                address.target(),
                graph.get(address),
                schema.identity_of(address),
            ) else {
                continue;
            };
            let Some(Value::Text(value)) = resource.attribute(attribute).and_then(Attribute::known)
            else {
                continue;
            };
            let key = (node.clone(), namespace.to_owned(), one_spelling(value));
            if let Some(first) = seen.get(&key) {
                return Err(PlanError::SameThing {
                    first: first.clone(),
                    second: address.clone(),
                    node: node.clone(),
                    namespace: namespace.to_owned(),
                    value: value.clone(),
                });
            }
            seen.insert(key, address.clone());
        }
    }
    Ok(())
}

/// What each removal takes away, and what each declared resource moving away
/// leaves, named as `name_of` names a declared resource: by machine, namespace and
/// one spelling of the value.
///
/// Taken from state, because the configuration that named them is gone or names
/// something else now. A record whose naming attribute is not a plain value names
/// nothing here.
#[dacc_derive::doc_anchor(id = "inv-plan-009-3")]
pub(super) fn taken_away(
    doomed: &[Vec<Address>],
    graph: &Graph,
    state: &State,
    schema: &Schema,
) -> BTreeMap<(Target, String, String), Vec<Address>> {
    let mut taken: BTreeMap<(Target, String, String), Vec<Address>> = BTreeMap::new();
    for address in doomed.iter().flatten() {
        if let Some(name) = recorded_name(address, state, schema) {
            taken.entry(name).or_default().push(address.clone());
        }
    }
    // And the name a declared resource moves away from, which the change that moves
    // it removes. A resource declared there was read there, adopted or updated from
    // the thing about to be deleted, and the move deleted it after: state recorded a
    // file that was not there.
    for address in state.addresses() {
        let (Some(resource), Some(recorded)) =
            (graph.get(address), recorded_name(address, state, schema))
        else {
            continue;
        };
        if name_of(resource, schema).is_some_and(|declared| declared != recorded) {
            taken.entry(recorded).or_default().push(address.clone());
        }
    }
    taken
}

/// What a record names on its machine, if its provider says and state kept the
/// value.
pub(super) fn recorded_name(
    address: &Address,
    state: &State,
    schema: &Schema,
) -> Option<(Target, String, String)> {
    let node = address.target()?;
    let (attribute, namespace) = schema.identity_of(address)?;
    let Value::Text(value) = state.get(address)?.attributes().get(attribute)?.plain()? else {
        return None;
    };
    Some((node.clone(), namespace.to_owned(), one_spelling(value)))
}

/// What a declared resource names on its machine, if its provider says and the
/// value is known when planning.
#[dacc_derive::doc_anchor(id = "inv-react-005-2")]
pub(super) fn name_of(resource: &Resource, schema: &Schema) -> Option<(Target, String, String)> {
    let address = resource.address();
    let node = address.target()?;
    let (attribute, namespace) = schema.identity_of(address)?;
    let Some(Value::Text(value)) = resource.attribute(attribute).and_then(Attribute::known) else {
        return None;
    };
    Some((node.clone(), namespace.to_owned(), one_spelling(value)))
}

/// Refuses a reaction on a kind whose provider says it cannot react.
///
/// Refused where it is written as well, when a configuration is evaluated. This
/// is for a graph that did not come from one.
pub(super) fn reactions_allowed(graph: &Graph, schema: &Schema) -> Result<(), PlanError> {
    for stage in graph.stages()? {
        for address in stage.addresses() {
            if let Some(resource) = graph.get(address)
                && !resource.reacts_to().is_empty()
                && !schema.reacts(address)
            {
                return Err(PlanError::CannotReact {
                    address: address.clone(),
                    kind: address.kind().as_str().to_owned(),
                });
            }
        }
    }
    Ok(())
}

/// A path without the spellings that name the same one: repeated separators, `.`
/// segments and a trailing separator. `..` is left as written, because through a
/// symbolic link it does not mean what it looks like.
pub(super) fn one_spelling(path: &str) -> String {
    let kept: Vec<&str> = path
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect();
    let joined = kept.join("/");
    if path.starts_with('/') {
        format!("/{joined}")
    } else {
        joined
    }
}

/// How old the newest thing known about a machine is, in seconds.
///
/// Taken from state, the only record of when nerpa last saw it. `None` means the
/// machine has never been touched, so there is nothing stale to be old.
pub(super) fn known_for(state: &State, node: &Target, now: Written) -> Option<u64> {
    state
        .addresses()
        .filter(|address| address.target() == Some(node))
        .filter_map(|address| state.get(address))
        .map(Record::written)
        .max()
        .map(|written| written.age(now).as_secs())
}
