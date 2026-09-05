//! The vocabulary a configuration is written in.
//!
//! Three functions, deliberately. `resource` declares something that should
//! exist, `node` says where a machine comes from, and `secret` says where a value
//! is kept without bringing it here. Everything else Starlark already provides —
//! lists, dictionaries, comprehensions, functions — and a module is just a
//! function, so there is no module system to invent.

use nerpa_core::secret::Source;
use nerpa_core::{Address, Attribute, Resource, Target, Value as CoreValue};
use starlark::environment::GlobalsBuilder;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::Value;
use starlark::values::dict::DictRef;
use starlark::values::list::ListRef;
use starlark::values::none::NoneType;

use crate::Declarations;
use crate::handle::{Handle, ResourceRef, SecretRef};

/// Anything a declaration can get wrong that the resource model refuses.
#[derive(Debug, thiserror::Error)]
enum Rejected {
    #[error("{0}")]
    Address(#[from] nerpa_core::AddressError),
    #[error(
        "attribute {name} of {address} holds {found}, which is not a value a resource can carry"
    )]
    Attribute {
        address: String,
        name: String,
        found: String,
    },
    #[error("provided_by expects a resource, and was given {0}")]
    NotAResource(String),
    #[error("secret() needs a store and a path, and {0} is empty")]
    NamelessSecret(&'static str),
}

fn declarations<'a>(evaluator: &Evaluator<'_, 'a, '_>) -> starlark::Result<&'a Declarations> {
    evaluator
        .extra
        .and_then(|extra| extra.downcast_ref::<Declarations>())
        .ok_or_else(|| {
            starlark::Error::new_other(std::io::Error::other("evaluation has no declaration store"))
        })
}

/// Turns one Starlark value into something a resource can carry.
fn attribute_of(value: Value<'_>) -> Option<Attribute> {
    if let Some(handle) = Handle::from_value(value) {
        return Some(Attribute::Unresolved(handle.reference().clone()));
    }
    if let Some(secret) = SecretRef::from_value(value) {
        return Some(Attribute::Secret(secret.source().clone()));
    }
    core_value_of(value).map(Attribute::Known)
}

fn core_value_of(value: Value<'_>) -> Option<CoreValue> {
    if let Some(text) = value.unpack_str() {
        return Some(CoreValue::Text(text.to_owned()));
    }
    if let Some(flag) = value.unpack_bool() {
        return Some(CoreValue::Boolean(flag));
    }
    if let Some(number) = value.unpack_i32() {
        return Some(CoreValue::Integer(i64::from(number)));
    }
    if let Some(items) = ListRef::from_value(value) {
        return items
            .iter()
            .map(core_value_of)
            .collect::<Option<Vec<_>>>()
            .map(CoreValue::List);
    }
    if let Some(entries) = DictRef::from_value(value) {
        return entries
            .iter()
            .map(|(key, item)| Some((key.unpack_str()?.to_owned(), core_value_of(item)?)))
            .collect::<Option<_>>()
            .map(CoreValue::Map);
    }
    None
}

#[starlark_module]
pub(crate) fn nerpa(builder: &mut GlobalsBuilder) {
    /// Declares that a resource should exist, and returns something to read it by.
    fn resource<'v>(
        #[starlark(require = pos)] address: &str,
        #[starlark(require = pos, default = NoneType)] attributes: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<ResourceRef> {
        let store = declarations(eval)?;
        let parsed: Address = address
            .parse()
            .map_err(|error| starlark::Error::new_value(Rejected::Address(error)))?;

        let mut resource = Resource::new(parsed.clone());
        if let Some(entries) = DictRef::from_value(attributes) {
            for (key, item) in entries.iter() {
                let Some(name) = key.unpack_str() else {
                    store.add_problem(format!("{parsed} has an attribute whose name is not text"));
                    continue;
                };
                match attribute_of(item) {
                    Some(attribute) => resource = resource.with(name, attribute),
                    None => store.add_problem(
                        Rejected::Attribute {
                            address: parsed.to_string(),
                            name: name.to_owned(),
                            found: item.get_type().to_owned(),
                        }
                        .to_string(),
                    ),
                }
            }
        }

        store.add_resource(resource);
        Ok(ResourceRef::new(parsed))
    }

    /// Names a value kept in a secret store, without reading it.
    ///
    /// What comes back can be given to a resource and nothing else: it cannot be
    /// compared, indexed, iterated or used as a condition. The value is fetched
    /// when a provider needs it, hashed for the plan, and never written down.
    fn secret(
        #[starlark(require = pos)] store: &str,
        #[starlark(require = pos)] path: &str,
        #[starlark(require = named, default = NoneType)] version: Value<'_>,
    ) -> starlark::Result<SecretRef> {
        // An empty path fetches nothing, and the failure would surface at plan
        // time as "no secret at lockbox:" — a long way from the line that wrote
        // it.
        for (what, text) in [("the store", store), ("the path", path)] {
            if text.is_empty() {
                return Err(starlark::Error::new_value(Rejected::NamelessSecret(what)));
            }
        }

        Ok(SecretRef::new(Source::Stored {
            provider: store.to_owned(),
            path: path.to_owned(),
            version: version.unpack_str().map(ToOwned::to_owned),
        }))
    }

    /// Declares a node, and what brings it into existence.
    ///
    /// Without `provided_by` the machine is taken to exist already, and nothing
    /// is ordered before touching it.
    fn node<'v>(
        #[starlark(require = pos)] name: &str,
        #[starlark(require = named, default = NoneType)] provided_by: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let store = declarations(eval)?;
        let target = Target::new(name)
            .map_err(|error| starlark::Error::new_value(Rejected::Address(error)))?;

        if provided_by.is_none() {
            store.add_node(target, None);
            return Ok(NoneType);
        }

        let Some(resource) = ResourceRef::from_value(provided_by) else {
            return Err(starlark::Error::new_value(Rejected::NotAResource(
                provided_by.get_type().to_owned(),
            )));
        };
        store.add_node(target, Some(resource.address().clone()));
        Ok(NoneType)
    }
}
