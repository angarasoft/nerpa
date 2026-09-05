//! What state remembers, and the order it hands things back in.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeSet;

use nerpa_core::address::address;
use nerpa_core::provider::Attributes;
use nerpa_core::secret::Recorded;
use nerpa_core::state::{Record, State};
use nerpa_core::{Address, Value};

fn record(depends_on: &[&str]) -> Record {
    let mut attributes = Attributes::new();
    attributes.insert("path".to_owned(), Value::Text("/somewhere".to_owned()));
    Record::new(
        Recorded::all(attributes),
        depends_on.iter().map(|text| address(text)).collect(),
    )
}

#[test]
fn state_reports_what_it_remembers() {
    let mut state = State::new();
    assert!(state.is_empty());
    assert_eq!(state.len(), 0);
    assert_eq!(state.get(&address("yandex:instance.db")), None);
    assert_eq!(state.addresses().count(), 0);

    state.record(address("yandex:instance.db"), record(&[]));
    assert!(!state.is_empty());
    assert_eq!(state.len(), 1);

    state.record(
        address("yandex:disk.spare"),
        record(&["yandex:instance.db"]),
    );
    assert_eq!(state.len(), 2);

    let remembered = state.get(&address("yandex:disk.spare")).expect("recorded");
    assert_eq!(
        remembered.attributes().get("path"),
        Some(&Recorded::Plain(Value::Text("/somewhere".to_owned())))
    );
    let read_from: Vec<String> = remembered
        .depends_on()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        read_from,
        ["yandex:instance.db"],
        "what it depended on is kept, not discarded"
    );

    state.forget(&address("yandex:disk.spare"));
    assert_eq!(state.len(), 1);
}

#[test]
fn nothing_is_abandoned_while_everything_is_still_declared() {
    let mut state = State::new();
    state.record(address("yandex:instance.db"), record(&[]));

    let db = address("yandex:instance.db");
    let declared: BTreeSet<&Address> = [&db].into_iter().collect();

    assert!(state.abandoned(&declared).is_empty());
}

#[test]
fn removals_come_back_in_reverse_dependency_order() {
    let mut state = State::new();
    state.record(address("yandex:network.main"), record(&[]));
    state.record(
        address("yandex:subnet.app"),
        record(&["yandex:network.main"]),
    );
    state.record(
        address("yandex:instance.web"),
        record(&["yandex:subnet.app"]),
    );

    let levels = state.abandoned(&BTreeSet::new());
    let order: Vec<Vec<String>> = levels
        .iter()
        .map(|level| level.iter().map(ToString::to_string).collect())
        .collect();

    assert_eq!(
        order,
        [
            vec!["yandex:instance.web".to_owned()],
            vec!["yandex:subnet.app".to_owned()],
            vec!["yandex:network.main".to_owned()],
        ],
        "each thing goes before what it read from"
    );
}

#[test]
fn something_still_declared_never_holds_up_a_removal() {
    let mut state = State::new();
    state.record(address("yandex:network.main"), record(&[]));
    state.record(
        address("yandex:subnet.app"),
        record(&["yandex:network.main"]),
    );

    let network = address("yandex:network.main");
    let declared: BTreeSet<&Address> = [&network].into_iter().collect();

    let levels = state.abandoned(&declared);
    assert_eq!(
        levels.len(),
        1,
        "the subnet does not wait for a network that is staying"
    );
    assert_eq!(levels[0], [address("yandex:subnet.app")]);
}

/// Recorded edges were applied in some order once, so a loop among them means
/// the record is inconsistent. Removing them together beats not at all.
#[test]
fn an_impossible_record_still_gives_an_answer() {
    let mut state = State::new();
    state.record(address("os:file.a@web-01"), record(&["os:file.b@web-01"]));
    state.record(address("os:file.b@web-01"), record(&["os:file.a@web-01"]));

    let levels = state.abandoned(&BTreeSet::new());
    assert_eq!(levels.len(), 1);
    assert_eq!(levels[0].len(), 2, "both go, together");
}
