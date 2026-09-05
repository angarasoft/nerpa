//! Values that will not exist until apply, and what they refuse to do.
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
use nerpa_core::{Address, Reference};
use starlark::any::ProvidesStaticType;
use starlark::starlark_simple_value;
use starlark::values::{Heap, NoSerialize, StarlarkValue, Value, starlark_value};

thread_local! {
    /// Handles asked for their truth value during the current evaluation.
    ///
    /// A thread local rather than shared state inside the value, because
    /// `to_bool` receives nothing but `&self`. Evaluation is single threaded and
    /// scoped to one call of [`crate::evaluate`], which clears this on the way in
    /// and drains it on the way out.
    static BRANCHED_ON: RefCell<Vec<Reference>> = const { RefCell::new(Vec::new()) };
}

/// Forgets any handles recorded by an earlier evaluation.
pub(crate) fn forget_branches() {
    BRANCHED_ON.with_borrow_mut(Vec::clear);
}

/// Everything branched on since [`forget_branches`], in the order it happened.
pub(crate) fn branches_taken() -> Vec<Reference> {
    BRANCHED_ON.with_borrow(Clone::clone)
}

/// What a handle says when it is asked to behave like a settled value.
#[derive(Debug, thiserror::Error)]
#[error(
    "{reference} will not exist until apply, so it cannot be used with {operation}; \
     pass it to another resource instead"
)]
struct Refused {
    reference: String,
    operation: &'static str,
}

fn refuse<T>(reference: &Reference, operation: &'static str) -> starlark::Result<T> {
    Err(starlark::Error::new_value(Refused {
        reference: reference.to_string(),
        operation,
    }))
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
        refuse(&self.reference, "==")
    }

    fn compare(&self, _other: Value<'v>) -> starlark::Result<std::cmp::Ordering> {
        refuse(&self.reference, "an ordering comparison")
    }

    fn at(&self, _index: Value<'v>, _heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        refuse(&self.reference, "indexing")
    }

    fn iterate_collect(&self, _heap: Heap<'v>) -> starlark::Result<Vec<Value<'v>>> {
        refuse(&self.reference, "iteration")
    }

    // ENFORCES: INV-PLAN-001
    fn to_bool(&self) -> bool {
        // The one refusal that cannot be made here. See the module documentation:
        // recorded now, and the whole evaluation is refused afterwards.
        BRANCHED_ON.with_borrow_mut(|taken| taken.push(self.reference.clone()));
        true
    }
}
