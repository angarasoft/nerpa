//! What the provider does when it cannot tell.
//!
//! Every case here used to read as "there is nothing at this path", which made
//! the plan propose creating a file that already existed and the apply overwrite
//! it — losing its contents and resetting its mode to `0644`. The tests are
//! written as a person would find these: on a real filesystem, with real
//! permissions, real symlinks and real bytes.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use nerpa_core::address::address;
use nerpa_core::observed::Facts;
use nerpa_core::provider::{
    Answered, Attributes, Concluded, Concluding, Planned, Prepared, Provider, ProviderError, Reach,
    Reading, Sending,
};
use nerpa_core::value::Value;
use nerpa_core::{Address, Observed};
use nerpa_proto::{Answer, Ask};
use nerpa_provider_os::Os;
use nerpa_transport::Here;

/// The executor, in this process: the same code a machine at the far end runs.
static HERE: Here = Here;

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
    os.read(
        &[Reading::new(target.clone(), at(path))],
        Observed::new().facts_of(None),
        &HERE,
    )
    .map(|found| found.attributes().get(&target).cloned())
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

/// A certificate is a file somebody manages. It used to read as absent, then
/// briefly as a refusal, and now as what it is.
#[test]
fn a_file_that_is_not_text_is_read_as_the_bytes_it_holds() {
    let scratch = Scratch::new("bytes");
    let path = scratch.join("cert.der");
    let der = [0xffu8, 0xfe, 0x00, 0x01];
    fs::write(&path, der).unwrap();

    let found = look(&path)
        .expect("bytes can be read")
        .expect("it is there");
    assert_eq!(
        found.get("content"),
        Some(&Value::Bytes(der.to_vec())),
        "{found:?}"
    );
}

/// And a file that *is* text still reads as text, or every diff of every
/// configuration file would turn into base64.
#[test]
fn a_file_that_is_text_is_still_read_as_text() {
    let scratch = Scratch::new("text");
    let path = scratch.join("nginx.conf");
    fs::write(&path, "listen 80;\n").unwrap();

    let found = look(&path).expect("text can be read").expect("it is there");
    assert_eq!(
        found.get("content"),
        Some(&Value::Text("listen 80;\n".to_owned())),
        "{found:?}"
    );
}

/// The whole loop, on bytes: written, read back, and the second run empty.
#[test]
fn a_binary_file_converges() {
    let scratch = Scratch::new("binary");
    let path = scratch.join("keytab");
    let bytes = vec![0x05u8, 0x02, 0x00, 0x00, 0xff, 0xfe, 0x80];

    let mut wanted = at(&path);
    wanted.insert("content".to_owned(), Value::Bytes(bytes.clone()));
    let written = applied(
        &address("os:file.subject@localhost"),
        Sending::Create,
        false,
        &wanted,
        Observed::new().facts_of(None),
        &HERE,
    )
    .expect("it writes");

    assert_eq!(
        fs::read(&path).unwrap(),
        bytes,
        "the bytes reached the disk"
    );
    assert_eq!(
        written.get("content"),
        Some(&Value::Bytes(bytes)),
        "and came back as themselves, so a second run has nothing to do"
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
    applied(
        &address("os:file.subject@localhost"),
        Sending::Update,
        false,
        &wanted,
        Observed::new().facts_of(None),
        &HERE,
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

    let refusal = removed(
        &address("os:directory.confd@localhost"),
        &at(&directory),
        Observed::new().facts_of(None),
        &HERE,
    )
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

    removed(
        &address("os:directory.confd@localhost"),
        &at(&directory),
        Observed::new().facts_of(None),
        &HERE,
    )
    .expect("an empty directory goes");
    assert!(!directory.exists());
}

/// The same, for a directory. `create_dir_all` treats a link to a directory as
/// already done, and `set_permissions` is `chmod`, which follows a link — so a
/// declared mode landed on whatever the link pointed at. The executor runs
/// privileged (0035), which makes that a `chmod` as root on a directory nothing
/// declared.
#[test]
fn making_a_directory_where_a_symlink_is_leaves_what_it_pointed_at_alone() {
    let scratch = Scratch::new("through-a-directory");
    let target = scratch.join("target");
    let link = scratch.join("declared");
    fs::create_dir(&target).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();

    let mut wanted = at(&link);
    wanted.insert("mode".to_owned(), Value::Text("0777".to_owned()));
    applied(
        &address("os:directory.subject@localhost"),
        Sending::Create,
        false,
        &wanted,
        Observed::new().facts_of(None),
        &HERE,
    )
    .expect("it makes the directory");

    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o700,
        "the directory the link pointed at was never declared and must not have \
         been given a mode"
    );
    assert!(
        !fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink(),
        "the declared path is a directory of its own now, not a pointer"
    );
}

/// A file ends up with the mode it was declared.
///
/// This is what a test can see. What it cannot see is the reason the code
/// changed: `fs::write` opened at `0o666` less the umask and the mode was
/// applied afterwards, so a file declared `0600` spent a moment readable by
/// everybody — and a file's content is one of the two places the protocol says
/// a secret goes. The file is opened with its mode now, which closes that by
/// construction; watching the window itself would need something outside this
/// process, and nothing here does.
///
/// The assertion is still worth making. The test written specifically to chase
/// a password through this path never looked at the mode at all.
#[test]
fn a_file_ends_up_with_the_mode_it_was_declared() {
    let scratch = Scratch::new("narrow");
    let path = scratch.join("credentials");

    let mut wanted = at(&path);
    wanted.insert("content".to_owned(), Value::Text("hunter2\n".to_owned()));
    wanted.insert("mode".to_owned(), Value::Text("0600".to_owned()));
    applied(
        &address("os:file.subject@localhost"),
        Sending::Create,
        false,
        &wanted,
        Observed::new().facts_of(None),
        &HERE,
    )
    .expect("it writes");

    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

/// A machine that answers a different number of things than it was asked about.
#[derive(Debug)]
struct Miscounting(usize);

impl Reach for Miscounting {
    fn ask(&self, _: &Ask) -> Result<Answer, ProviderError> {
        Ok(Answer::Found {
            seen: vec![nerpa_proto::Seen::Nothing; self.0],
        })
    }
}

/// Answers are read by position, so a count that does not match is the two ends
/// having lost step. One too few would leave the last file described by nothing,
/// which reads as absent and gets it created again; one too many would describe
/// each file by its neighbour. Neither is an answer, and review found nothing
/// counted.
#[test]
fn a_reading_answered_with_the_wrong_number_of_things_is_refused() {
    let scratch = Scratch::new("miscount");
    let batch = [
        Reading::new(address("os:file.one@localhost"), at(&scratch.join("one"))),
        Reading::new(address("os:file.two@localhost"), at(&scratch.join("two"))),
    ];

    for answered in [1, 3] {
        let refusal = Os::new()
            .read(
                &batch,
                Observed::new().facts_of(None),
                &Miscounting(answered),
            )
            .expect_err("a miscounted answer is not a reading");
        let said = refusal.to_string();
        assert!(
            said.contains(&format!("answered {answered} things about 2")),
            "{said}"
        );
    }
}

/// A machine that must not be asked anything.
#[derive(Debug)]
struct Untouchable;

impl Reach for Untouchable {
    fn ask(&self, _: &Ask) -> Result<Answer, ProviderError> {
        panic!("the machine was asked, and nothing should have been sent")
    }
}

/// The os provider says how much content one operation carries, so the planner
/// can refuse more before anything is done.
#[test]
fn the_os_provider_bounds_content_at_what_one_operation_carries() {
    assert_eq!(
        Os::new()
            .schema()
            .bound(&address("os:file.motd@localhost"), "content"),
        Some(nerpa_proto::MAX_CONTENT)
    );
}

/// Content over the bound is refused before the machine is asked. Planning sees
/// every value but a secret's; this is where a secret's is caught, and where any
/// value would be caught if planning were bypassed.
#[test]
fn content_too_large_is_refused_before_the_machine_is_asked() {
    let scratch = Scratch::new("too-large");
    let mut attributes = at(&scratch.join("big"));
    attributes.insert(
        "content".to_owned(),
        Value::Text("x".repeat(nerpa_proto::MAX_CONTENT + 1)),
    );

    let refusal = applied(
        &address("os:file.big@localhost"),
        Sending::Create,
        false,
        &attributes,
        Observed::new().facts_of(None),
        &Untouchable,
    )
    .expect_err("over the bound");
    assert!(
        refusal.to_string().contains("most one operation carries"),
        "{refusal}"
    );
}

/// A file and a directory at one path are one thing on the machine, so the os
/// provider names both by their path, in one namespace.
#[test]
fn the_os_provider_names_files_and_directories_by_path_in_one_namespace() {
    let schema = Os::new().schema();
    assert_eq!(
        schema.identity_of(&address("os:file.a@localhost")),
        Some(("path", "path"))
    );
    assert_eq!(
        schema.identity_of(&address("os:directory.b@localhost")),
        Some(("path", "path"))
    );
}

/// One change, the way the engine does it: prepared, sent as one batch, and
/// concluded from the answers. A provider does not send anything itself.
fn applied(
    address: &Address,
    kind: Sending,
    roused: bool,
    attributes: &Attributes,
    facts: Facts<'_>,
    reach: &dyn Reach,
) -> Result<Attributes, ProviderError> {
    let os = Os::new();
    let planned = os.prepare(&Prepared {
        address,
        kind,
        roused,
        resent: false,
        desired: attributes,
        recorded: None,
        facts,
        secret: &BTreeSet::new(),
    })?;
    let became = answered(planned, address, reach)?;
    let secret = BTreeSet::new();
    match os.conclude(
        &Concluding {
            address,
            kind,
            roused,
            facts,
            plain: attributes,
            secret: &secret,
        },
        &became,
    )? {
        Concluded::Became(attributes) => Ok(attributes),
        Concluded::Gone => Ok(Attributes::new()),
    }
}

/// The same for a removal, which is built from what state recorded.
fn removed(
    address: &Address,
    recorded: &Attributes,
    facts: Facts<'_>,
    reach: &dyn Reach,
) -> Result<(), ProviderError> {
    let os = Os::new();
    let nothing = Attributes::new();
    let planned = os.prepare(&Prepared {
        address,
        kind: Sending::Delete,
        roused: false,
        resent: false,
        desired: &nothing,
        recorded: Some(recorded),
        facts,
        secret: &BTreeSet::new(),
    })?;
    let became = answered(planned, address, reach)?;
    let secret = BTreeSet::new();
    os.conclude(
        &Concluding {
            address,
            kind: Sending::Delete,
            roused: false,
            facts,
            plain: recorded,
            secret: &secret,
        },
        &became,
    )
    .map(|_| ())
}

/// What the far end said about each operation, counted as the engine counts it.
fn answered(
    planned: Planned,
    address: &Address,
    reach: &dyn Reach,
) -> Result<Vec<Answered>, ProviderError> {
    let asked = planned.operations.len();
    let answer = reach.ask(&Ask::Perform {
        plan: None,
        premise: Vec::new(),
        operations: planned.operations,
        seal: None,
    })?;
    let seen = match answer {
        Answer::Done { became } | Answer::Found { seen: became } if became.len() == asked => became,
        Answer::Failed { at, problem } => {
            return Err(ProviderError::World {
                address: address.clone(),
                problem: format!("at {at}: {problem}"),
            });
        }
        other => {
            return Err(ProviderError::World {
                address: address.clone(),
                problem: format!("the far end said something unexpected: {other:?}"),
            });
        }
    };
    Ok(seen.into_iter().map(Answered::Saw).collect())
}
