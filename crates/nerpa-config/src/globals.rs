//! The vocabulary a configuration is written in.
//!
//! Three functions, deliberately. `resource` declares something that should
//! exist, `node` says where a machine comes from, and `secret` says where a value
//! is kept without bringing it here. Everything else Starlark already provides —
//! lists, dictionaries, comprehensions, functions — and a module is just a
//! function, so there is no module system to invent.

use nerpa_core::attribute::Choice;
use nerpa_core::secret::Source;
use nerpa_core::{Address, Attribute, Resource, Target, Value as CoreValue};
use starlark::environment::GlobalsBuilder;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::Value;
use starlark::values::bytes::StarlarkBytes;
use starlark::values::dict::DictRef;
use starlark::values::list::ListRef;
use starlark::values::none::NoneType;

use crate::Declarations;
use crate::handle::{ChoiceRef, FactRef, Handle, NodeRef, ResourceRef, SecretRef};

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
    #[error(
        "reacts_to of {address} holds {found}, and a reaction is declared by \
         reference — `reacts_to = [conf]`, where `conf` is what `resource()` \
         gave back"
    )]
    ReactsToNotAResource { address: String, found: String },
    #[error("{0} was declared to react to itself")]
    ReactsToItself(String),
    #[error("{0} is given something to react to, and its provider says this kind cannot react")]
    CannotReact(String),
    #[error("secret() needs a store and a path, and {0} is empty")]
    NamelessSecret(&'static str),
    #[error("template() takes a name and a dictionary of values, and was given {0}")]
    NotValues(String),
    #[error(
        "select() chooses by something read from a machine — `select(web.os_family, {{...}})` — \
         and was given {0}"
    )]
    NotAFact(String),
    #[error("select() needs a dictionary of what it could be, and was given {0}")]
    NotOptions(String),
    #[error("select() was given nothing to choose between")]
    NoOptions,
    #[error("the fleet describes no group called {0:?}")]
    NoSuchGroup(String),
    #[error("{0}")]
    Template(String),
}

fn declarations<'a>(evaluator: &Evaluator<'_, 'a, '_>) -> starlark::Result<&'a Declarations> {
    evaluator
        .extra
        .and_then(|extra| extra.downcast_ref::<Declarations>())
        .ok_or_else(|| {
            starlark::Error::new_other(std::io::Error::other("evaluation has no declaration store"))
        })
}

/// A machine, carrying what the fleet says about it.
fn node_ref(store: &Declarations, node: &Target) -> NodeRef {
    NodeRef::new(node.clone(), store.fleet().values_of(node))
}

/// Turns one Starlark value into something a resource can carry.
fn attribute_of(value: Value<'_>) -> Option<Attribute> {
    if let Some(handle) = Handle::from_value(value) {
        return Some(Attribute::Unresolved(handle.reference().clone()));
    }
    if let Some(secret) = SecretRef::from_value(value) {
        return Some(Attribute::Secret(secret.source().clone()));
    }
    if let Some(choice) = ChoiceRef::from_value(value) {
        return Some(Attribute::Chosen(choice.choice().clone()));
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
    if let Some(bytes) = StarlarkBytes::from_value(value) {
        return Some(CoreValue::bytes(bytes.as_bytes().to_vec()));
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
    #[dacc_derive::doc_anchor(id = "inv-react-005-1")]
    fn resource<'v>(
        #[starlark(require = pos)] address: &str,
        #[starlark(require = pos, default = NoneType)] attributes: Value<'v>,
        #[starlark(require = named, default = NoneType)] reacts_to: Value<'v>,
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

        // Declared by reference rather than by name, which is the whole of
        // decision 0026: a typo is this error with a line number instead of
        // silence, and two modules cannot collide over "restart nginx".
        if !reacts_to.is_none() {
            for item in reacts_to.iterate(eval.heap())? {
                // Refused where it is written. A package had nothing a change
                // elsewhere could ask of it: a reaction did nothing on rhel and
                // merged it again on Portage.
                if !store.schema().reacts(&parsed) {
                    return Err(starlark::Error::new_value(Rejected::CannotReact(
                        parsed.to_string(),
                    )));
                }
                let Some(cause) = ResourceRef::from_value(item) else {
                    return Err(starlark::Error::new_value(Rejected::ReactsToNotAResource {
                        address: parsed.to_string(),
                        found: item.get_type().to_owned(),
                    }));
                };
                if cause.address() == &parsed {
                    return Err(starlark::Error::new_value(Rejected::ReactsToItself(
                        parsed.to_string(),
                    )));
                }
                resource = resource.reacting_to(cause.address().clone());
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

    /// Declares which nerpa build this configuration expects; a different build
    /// refuses rather than behaving differently (decision 0022).
    #[dacc_derive::doc_anchor(id = "config-requires-version")]
    fn requires<'v>(
        #[starlark(require = pos)] version: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        declarations(eval)?.require_version(version);
        Ok(NoneType)
    }

    /// Declares a rename: the resource at `from` is now declared at `to`, and
    /// its identity — the thing on the machine — does not change (decision
    /// 0028). Without this, a rename reads as a delete and a create.
    fn moved<'v>(
        #[starlark(require = pos)] from: &str,
        #[starlark(require = pos)] to: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let store = declarations(eval)?;
        let from: Address = from
            .parse()
            .map_err(|error| starlark::Error::new_value(Rejected::Address(error)))?;
        let to: Address = to
            .parse()
            .map_err(|error| starlark::Error::new_value(Rejected::Address(error)))?;
        store.add_moved(from, to);
        Ok(NoneType)
    }

    /// Renders a template from the configuration's own directory.
    ///
    /// The values it may see are the ones handed to it here and nothing else.
    /// There is no ambient scope, no clock, no environment and no way to read a
    /// file — a template that reads the world renders differently every time,
    /// and a plan computed from it could not be compared or signed.
    ///
    /// Rendered now rather than later, so what comes back is text, and the plan
    /// shows the file's contents before anything happens.
    fn template<'v>(
        #[starlark(require = pos)] name: &str,
        #[starlark(require = pos, default = NoneType)] values: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<String> {
        let store = declarations(eval)?;

        let mut handed = std::collections::BTreeMap::new();
        if !values.is_none() {
            let Some(entries) = DictRef::from_value(values) else {
                return Err(starlark::Error::new_value(Rejected::NotValues(
                    values.get_type().to_owned(),
                )));
            };
            for (key, item) in entries.iter() {
                let Some(key) = key.unpack_str() else {
                    return Err(starlark::Error::new_value(Rejected::NotValues(
                        "a dictionary with a key that is not text".to_owned(),
                    )));
                };
                let Some(value) = core_value_of(item) else {
                    return Err(starlark::Error::new_value(Rejected::Template(format!(
                        "value {key:?} for template {name:?} holds {}, which a template \
                         cannot be shown",
                        item.get_type()
                    ))));
                };
                handed.insert(key.to_owned(), value);
            }
        }

        store
            .renderer()
            .render(name, &handed)
            .map_err(|problem| starlark::Error::new_value(Rejected::Template(problem)))
    }

    /// Chooses a value by something a machine will turn out to be.
    ///
    /// Every branch is written here, so the graph is known whichever way it
    /// goes: one resource, one edge, one place in the order. Only the value is
    /// open, and only within this set — which the plan can print, and a person
    /// can approve. A machine that says something no branch accounts for is
    /// refused, not defaulted.
    ///
    /// This is the whole mechanism decision 0029 allows. A fact may decide what
    /// an attribute says and never whether a resource exists, which is why there
    /// is no `if` that a fact can reach.
    fn select<'v>(
        #[starlark(require = pos)] by: Value<'v>,
        #[starlark(require = pos)] options: Value<'v>,
    ) -> starlark::Result<ChoiceRef> {
        let Some(fact) = FactRef::from_value(by) else {
            return Err(starlark::Error::new_value(Rejected::NotAFact(
                by.get_type().to_owned(),
            )));
        };
        let Some(entries) = DictRef::from_value(options) else {
            return Err(starlark::Error::new_value(Rejected::NotOptions(
                options.get_type().to_owned(),
            )));
        };

        let mut among = std::collections::BTreeMap::new();
        for (said, item) in entries.iter() {
            let Some(said) = said.unpack_str() else {
                return Err(starlark::Error::new_value(Rejected::NotOptions(
                    "a dictionary keyed by something other than text".to_owned(),
                )));
            };
            let Some(value) = core_value_of(item) else {
                return Err(starlark::Error::new_value(Rejected::Attribute {
                    address: fact.fact().to_string(),
                    name: said.to_owned(),
                    found: item.get_type().to_owned(),
                }));
            };
            among.insert(said.to_owned(), value);
        }

        if among.is_empty() {
            return Err(starlark::Error::new_value(Rejected::NoOptions));
        }
        Ok(ChoiceRef::new(Choice::among(fact.fact().clone(), among)))
    }

    /// Declares a node, and what brings it into existence.
    ///
    /// Without `provided_by` the machine is taken to exist already, and nothing
    /// is ordered before touching it.
    fn node<'v>(
        #[starlark(require = pos)] name: &str,
        #[starlark(require = named, default = NoneType)] provided_by: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NodeRef> {
        let store = declarations(eval)?;
        let target = Target::new(name)
            .map_err(|error| starlark::Error::new_value(Rejected::Address(error)))?;

        if provided_by.is_none() {
            store.add_node(target.clone(), None);
            return Ok(node_ref(store, &target));
        }

        let Some(resource) = ResourceRef::from_value(provided_by) else {
            return Err(starlark::Error::new_value(Rejected::NotAResource(
                provided_by.get_type().to_owned(),
            )));
        };
        store.add_node(target.clone(), Some(resource.address().clone()));
        Ok(node_ref(store, &target))
    }

    /// Every machine in a group, as the fleet document has it.
    ///
    /// Iterable, because the inventory is data by the time the program sees it:
    /// which machines exist is decided by a person or by the cloud stage and is
    /// knowable before evaluation, while what a machine turned out to be is not.
    /// That line is the whole of decision 0025, and it is why a hundred machines
    /// need not be a hundred declarations.
    fn group<'v>(
        #[starlark(require = pos)] name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Vec<NodeRef>> {
        let store = declarations(eval)?;
        let Some(members) = store.fleet().group(name) else {
            return Err(starlark::Error::new_value(Rejected::NoSuchGroup(
                name.to_owned(),
            )));
        };

        // Registered as they are handed out rather than all at once when the
        // fleet is read: a document describing two hundred machines should not
        // make a run that touches three open two hundred connections.
        Ok(members
            .into_iter()
            .map(|node| {
                store.add_node(node.clone(), None);
                node_ref(store, &node)
            })
            .collect())
    }
}
