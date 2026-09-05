//! What the provider does when it cannot tell.
//!
//! Every case here used to read as "there is nothing at this path", which made
//! the plan propose creating a file that already existed and the apply overwrite
//! it — losing its contents and resetting its mode to `0644`. The tests are
//! written as a person would find these: on a real filesystem, with real
//! permissions, real symlinks and real bytes.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use nerpa_core::address::address;
use nerpa_core::plan::ChangeKind;
use nerpa_core::provider::{Attributes, Provider, Reading};
use nerpa_core::value::Value;
use nerpa_provider_os::Os;

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "nerpa-unreadable-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("a scratch directory");
        Self(path)
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        // Modes are put back first: a directory this test made unreadable would
        // otherwise survive the run as litter.
        if let Ok(entries) = fs::read_dir(&self.0) {
            for entry in entries.flatten() {
                let _ = fs::set_permissions(entry.path(), fs::Permissions::from_mode(0o644));
            }
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn at(path: &Path) -> Attributes {
    let mut attributes = Attributes::new();
    attributes.insert("path".to_owned(), Value::Text(path.display().to_string()));
    attributes
}

/// What the provider reports for one path.
fn look(path: &Path) -> Result<Option<Attributes>, String> {
    let os = Os::new();
    let target = address("os:file.subject@localhost");
    os.read(&[Reading::new(target.clone(), at(path))])
        .map(|found| found.get(&target).cloned())
        .map_err(|error| error.to_string())
}

/// The one that loses data. A file nerpa may not open is not a file that is not
/// there, and the difference is the whole plan.
#[test]
fn a_file_that_cannot_be_read_is_refused_rather_than_called_absent() {
    let scratch = Scratch::new("noperm");
    let path = scratch.join("credentials");
    fs::write(&path, "the original contents").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();

    assert!(
        fs::read_to_string(&path).is_err(),
        "this test needs a file it cannot read; running as root?"
    );

    let refusal = look(&path).expect_err("an unreadable file is not an absent one");
    assert!(
        refusal.contains("Permission denied") && refusal.contains("credentials"),
        "the refusal should name the path and what the system said: {refusal}"
    );

    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "the original contents",
        "nothing should have been written over it"
    );
}

/// `symlink_metadata` describes the link and reading the path describes its
/// target. Reporting the mode of one and the contents of the other is a record
/// of an object that does not exist.
#[test]
fn a_symlink_is_reported_as_a_symlink_and_not_followed() {
    let scratch = Scratch::new("symlink");
    let target = scratch.join("target.txt");
    let link = scratch.join("link.conf");
    fs::write(&target, "belongs to the target").unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();

    let found = look(&link)
        .expect("a symlink can be looked at")
        .expect("it is there");
    assert_eq!(
        found.get("symlink_to"),
        Some(&Value::Text(target.display().to_string())),
        "it should say what it points at: {found:?}"
    );
    assert!(
        !found.contains_key("content"),
        "the target's contents are not this path's contents: {found:?}"
    );
}

#[test]
fn a_broken_symlink_is_there_rather_than_absent() {
    let scratch = Scratch::new("broken");
    let link = scratch.join("link.conf");
    std::os::unix::fs::symlink(scratch.join("nothing-here"), &link).unwrap();

    let found = look(&link).expect("a broken symlink can still be looked at");
    assert!(
        found.is_some(),
        "a link that points nowhere is still a thing at this path"
    );
}

#[test]
fn a_file_that_is_not_utf8_is_refused_rather_than_called_absent() {
    let scratch = Scratch::new("bytes");
    let path = scratch.join("cert.der");
    fs::write(&path, [0xff, 0xfe, 0x00, 0x01]).unwrap();

    let refusal = look(&path).expect_err("bytes are not nothing");
    assert!(
        refusal.contains("UTF-8") && refusal.contains("cert.der"),
        "the refusal should say what it could not represent: {refusal}"
    );
}

/// Writing through a link changes a file the declaration does not name.
#[test]
fn writing_where_a_symlink_is_replaces_the_link_and_leaves_its_target_alone() {
    let scratch = Scratch::new("through");
    let target = scratch.join("target.txt");
    let link = scratch.join("link.conf");
    fs::write(&target, "belongs to the target").unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();

    let mut wanted = at(&link);
    wanted.insert(
        "content".to_owned(),
        Value::Text("declared here".to_owned()),
    );
    Os::new()
        .apply(
            &address("os:file.subject@localhost"),
            ChangeKind::Update,
            &wanted,
        )
        .expect("it writes");

    assert_eq!(fs::read_to_string(&link).unwrap(), "declared here");
    assert_eq!(
        fs::read_to_string(&target).unwrap(),
        "belongs to the target",
        "the file the link pointed at was never declared and must not have moved"
    );
    assert!(
        !fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink(),
        "the path should now hold the file that was declared"
    );
}

/// A directory nerpa made may since have been filled by somebody else.
/// Recursively deleting what it never created is a second, unannounced change.
#[test]
fn deleting_a_directory_does_not_take_what_nerpa_did_not_make() {
    let scratch = Scratch::new("rmdir");
    let directory = scratch.join("conf.d");
    fs::create_dir_all(&directory).unwrap();
    let stranger = directory.join("written-by-somebody-else.conf");
    fs::write(&stranger, "not ours").unwrap();

    let refusal = Os::new()
        .delete(&address("os:directory.confd@localhost"), &at(&directory))
        .expect_err("a directory with something in it is not empty");
    assert!(
        refusal.to_string().contains("conf.d"),
        "the refusal should name the directory: {refusal}"
    );
    assert_eq!(
        fs::read_to_string(&stranger).unwrap(),
        "not ours",
        "the stranger's file must still be there"
    );
}

#[test]
fn an_empty_directory_is_still_removed() {
    let scratch = Scratch::new("rmdir-empty");
    let directory = scratch.join("conf.d");
    fs::create_dir_all(&directory).unwrap();

    Os::new()
        .delete(&address("os:directory.confd@localhost"), &at(&directory))
        .expect("an empty directory goes");
    assert!(!directory.exists());
}
