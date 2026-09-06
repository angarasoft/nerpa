//! Values a configuration may pass around but must not look at.
//!
//! Evaluating a configuration produces a graph before anything has been created,
//! so an address the cloud has not allocated yet cannot be a number. It is a
//! handle: it can be passed around and stored in another resource's attributes,
//! and that is all.
//!
//! # What it refuses, and how well
//!
//! Comparison, ordering, indexing and iteration all return an error through
//! Starlark's own machinery, so the failure names the file and line.
//!
//! Truthiness cannot. `StarlarkValue::to_bool` returns a plain `bool` with no
//! error channel and no access to the evaluator, so `if db.ip:` cannot be
//! stopped where it happens. Instead the handle records that it was asked, and
//! [`crate::evaluate`] refuses the whole run afterwards — the graph is not
//! returned, so the invariant still holds. The diagnostic is weaker: it names
//! the value rather than the line.

use std::cell::RefCell;
use std::fmt;

use allocative::Allocative;
use nerpa_core::attribute::{Choice, Fact};
use nerpa_core::secret::Source;
use nerpa_core::{Address, Reference, Target};
use starlark::any::ProvidesStaticType;
use starlark::starlark_simple_value;
use starlark::values::{Heap, NoSerialize, StarlarkValue, Value, starlark_value};

thread_local! {
    /// Values asked for their truth value during the current evaluation.
    ///
    /// A thread local rather than shared state inside the value, because
    /// `to_bool` receives nothing but `&self`. Evaluation is single threaded and
    /// scoped to one call of [`crate::evaluate`], which clears this on the way in
    /// and drains it on the way out.
    static BRANCHED_ON: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Forgets any handles recorded by an earlier evaluation.
pub(crate) fn forget_branches() {
    BRANCHED_ON.with_borrow_mut(Vec::clear);
}

/// Everything branched on since [`forget_branches`], in the order it happened.
pub(crate) fn branches_taken() -> Vec<String> {
    BRANCHED_ON.with_borrow(Clone::clone)
}

/// What an opaque value says when it is asked to behave like a settled one.
#[derive(Debug, thiserror::Error)]
#[error("{subject} cannot be used with {operation}; pass it to another resource instead")]
struct Refused {
    subject: String,
    operation: &'static str,
}

fn refuse<T>(subject: String, operation: &'static str) -> starlark::Result<T> {
    Err(starlark::Error::new_value(Refused { subject, operation }))
}

/// A resource, as the configuration refers to it.
#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub(crate) struct ResourceRef {
    // Skipped rather than derived: `Allocative` exists for heap profiling, and
    // teaching nerpa-core about it would put a Starlark-ecosystem dependency in
    // the one crate that must not have any.
    #[allocative(skip)]
    address: Address,
}

impl ResourceRef {
    pub(crate) fn new(address: Address) -> Self {
        Self { address }
    }

    pub(crate) fn address(&self) -> &Address {
        &self.address
    }
}

impl fmt::Display for ResourceRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.address)
    }
}

starlark_simple_value!(ResourceRef);

#[starlark_value(type = "resource")]
impl<'v> StarlarkValue<'v> for ResourceRef {
    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        Some(heap.alloc(Handle::new(Reference::to(self.address.clone(), attribute))))
    }

    fn has_attr(&self, _attribute: &str, _heap: Heap<'v>) -> bool {
        // Every attribute is answerable: what a resource will expose is decided
        // by its provider, and none of that is known here.
        true
    }
}

/// A value that will only exist once something has been created.
#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub(crate) struct Handle {
    #[allocative(skip)]
    reference: Reference,
}

impl Handle {
    pub(crate) fn new(reference: Reference) -> Self {
        Self { reference }
    }

    pub(crate) fn reference(&self) -> &Reference {
        &self.reference
    }
}

impl fmt::Display for Handle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{}, unknown until apply>", self.reference)
    }
}

starlark_simple_value!(Handle);

#[starlark_value(type = "unresolved")]
impl<'v> StarlarkValue<'v> for Handle {
    fn equals(&self, _other: Value<'v>) -> starlark::Result<bool> {
        refuse(self.to_string(), "==")
    }

    fn compare(&self, _other: Value<'v>) -> starlark::Result<std::cmp::Ordering> {
        refuse(self.to_string(), "an ordering comparison")
    }

    fn at(&self, _index: Value<'v>, _heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        refuse(self.to_string(), "indexing")
    }

    fn iterate_collect(&self, _heap: Heap<'v>) -> starlark::Result<Vec<Value<'v>>> {
        refuse(self.to_string(), "iteration")
    }

    // ENFORCES: INV-PLAN-001
    fn to_bool(&self) -> bool {
        // The one refusal that cannot be made here. See the module documentation:
        // recorded now, and the whole evaluation is refused afterwards.
        BRANCHED_ON.with_borrow_mut(|taken| taken.push(self.reference.to_string()));
        true
    }
}

/// A secret, as the configuration refers to it.
///
/// It says where the value is kept and never brings it here. Everything a handle
/// refuses, this refuses for a sharper reason: `password == "admin"` is not a
/// mistake about ordering, it is an oracle, and a configuration is not a place to
/// build one. See decision 0027.
#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub(crate) struct SecretRef {
    #[allocative(skip)]
    source: Source,
}

impl SecretRef {
    pub(crate) fn new(source: Source) -> Self {
        Self { source }
    }

    pub(crate) fn source(&self) -> &Source {
        &self.source
    }
}

impl fmt::Display for SecretRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<secret from {}>", self.source)
    }
}

starlark_simple_value!(SecretRef);

#[starlark_value(type = "secret")]
impl<'v> StarlarkValue<'v> for SecretRef {
    fn equals(&self, _other: Value<'v>) -> starlark::Result<bool> {
        refuse(self.to_string(), "==")
    }

    fn compare(&self, _other: Value<'v>) -> starlark::Result<std::cmp::Ordering> {
        refuse(self.to_string(), "an ordering comparison")
    }

    fn at(&self, _index: Value<'v>, _heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        refuse(self.to_string(), "indexing")
    }

    fn iterate_collect(&self, _heap: Heap<'v>) -> starlark::Result<Vec<Value<'v>>> {
        refuse(self.to_string(), "iteration")
    }

    // ENFORCES: INV-SECRET-004
    fn to_bool(&self) -> bool {
        BRANCHED_ON.with_borrow_mut(|taken| taken.push(self.to_string()));
        true
    }
}

/// A machine, as the configuration refers to it.
#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub(crate) struct NodeRef {
    #[allocative(skip)]
    node: Target,
}

impl NodeRef {
    pub(crate) fn new(node: Target) -> Self {
        Self { node }
    }
}

impl fmt::Display for NodeRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.node)
    }
}

starlark_simple_value!(NodeRef);

#[starlark_value(type = "node")]
impl<'v> StarlarkValue<'v> for NodeRef {
    /// `web.os_family` is a fact of that machine.
    ///
    /// The same spelling as reading an attribute of a resource, and the same
    /// opacity: both are things this configuration cannot know while it is being
    /// read. Which one it is shows in what it refuses and in what the plan says.
    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        Some(heap.alloc(FactRef::new(Fact::of(self.node.clone(), attribute))))
    }

    fn has_attr(&self, _attribute: &str, _heap: Heap<'v>) -> bool {
        // Which facts a machine has is decided by the machine, and none of that
        // is known here.
        true
    }
}

/// Something a machine will turn out to be.
///
/// It may be given to `select` and to nothing else. Compared, ordered, indexed
/// or used as a condition it refuses, for the reason decision 0029 gives: a fact
/// may decide what an attribute says and never whether a resource exists.
#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub(crate) struct FactRef {
    #[allocative(skip)]
    fact: Fact,
}

impl FactRef {
    pub(crate) fn new(fact: Fact) -> Self {
        Self { fact }
    }

    pub(crate) fn fact(&self) -> &Fact {
        &self.fact
    }
}

impl fmt::Display for FactRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{}, read from the machine>", self.fact)
    }
}

starlark_simple_value!(FactRef);

#[starlark_value(type = "fact")]
impl<'v> StarlarkValue<'v> for FactRef {
    fn equals(&self, _other: Value<'v>) -> starlark::Result<bool> {
        refuse(self.to_string(), "==")
    }

    fn compare(&self, _other: Value<'v>) -> starlark::Result<std::cmp::Ordering> {
        refuse(self.to_string(), "an ordering comparison")
    }

    fn at(&self, _index: Value<'v>, _heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        refuse(self.to_string(), "indexing")
    }

    fn iterate_collect(&self, _heap: Heap<'v>) -> starlark::Result<Vec<Value<'v>>> {
        refuse(self.to_string(), "iteration")
    }

    // ENFORCES: INV-FACT-001
    fn to_bool(&self) -> bool {
        BRANCHED_ON.with_borrow_mut(|taken| taken.push(self.to_string()));
        true
    }
}

/// A value chosen by a fact, as the configuration holds it.
///
/// It refuses everything a fact refuses, for the same reason: what it will be is
/// not known here, and the only thing that may be done with it is to give it to
/// a resource.
#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub(crate) struct ChoiceRef {
    #[allocative(skip)]
    choice: Choice,
}

impl ChoiceRef {
    pub(crate) fn new(choice: Choice) -> Self {
        Self { choice }
    }

    pub(crate) fn choice(&self) -> &Choice {
        &self.choice
    }
}

impl fmt::Display for ChoiceRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{}>", self.choice)
    }
}

starlark_simple_value!(ChoiceRef);

#[starlark_value(type = "choice")]
impl<'v> StarlarkValue<'v> for ChoiceRef {
    fn equals(&self, _other: Value<'v>) -> starlark::Result<bool> {
        refuse(self.to_string(), "==")
    }

    fn compare(&self, _other: Value<'v>) -> starlark::Result<std::cmp::Ordering> {
        refuse(self.to_string(), "an ordering comparison")
    }

    fn at(&self, _index: Value<'v>, _heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        refuse(self.to_string(), "indexing")
    }

    fn iterate_collect(&self, _heap: Heap<'v>) -> starlark::Result<Vec<Value<'v>>> {
        refuse(self.to_string(), "iteration")
    }

    // ENFORCES: INV-FACT-001
    fn to_bool(&self) -> bool {
        BRANCHED_ON.with_borrow_mut(|taken| taken.push(self.to_string()));
        true
    }
}
