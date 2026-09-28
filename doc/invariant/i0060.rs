use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-REACT-005"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("A provider that does not say its kind cannot react, or a graph planned with a schema that does not carry the provider's. Either plans a reaction that does whatever the provider's apply does for that kind."), tests: &[crate::anchor::a_kind_that_cannot_react_is_refused_where_the_reaction_is_written, crate::anchor::a_kind_that_cannot_react_is_refused_when_planning] }),
    statement: NonEmptyStr::new("A kind whose provider says it cannot react is never given a reaction."),
    rationale: r#"A reaction is something a change elsewhere asks a resource to do again, and a package has nothing to do again: on rhel a reaction did nothing, and on Portage it merged the package again. A provider says which of its kinds cannot react. A configuration giving one `reacts_to` is refused at the line that says it, and a graph that did not come from a configuration is refused when planning. Decided after the third review of RFC 0001."#,
    enforced_by: &[crate::anchor::inv_react_005_1, crate::anchor::inv_react_005_2],
    tests: &[crate::anchor::a_kind_that_cannot_react_is_refused_where_the_reaction_is_written, crate::anchor::a_kind_that_cannot_react_is_refused_when_planning, crate::anchor::the_os_provider_says_a_package_cannot_react, crate::anchor::what_can_react_still_does],
);
