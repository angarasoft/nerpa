//! A WASM provider holds a credential as a handle, never a value, and its
//! requests reach only the hosts its manifest declares.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeSet;
use std::fs;
use std::io::{Read as _, Write as _};
use std::path::PathBuf;

use nerpa_core::address::{Target, address};
use nerpa_core::observed::Observed;
use nerpa_core::provider::{Prepared, Provider as ProviderTrait, ProviderError, Sending};
use nerpa_core::value::Value;
use nerpa_provider_wasm::Wasm;

/// A directory that removes itself.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("nerpa-wasm-{name}-{}", std::process::id()));
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
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The checked-in core module, wrapped into a component by the encoder.
fn component() -> Vec<u8> {
    let core = fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/guest_file_provider.wasm"
    ))
    .expect("the core module");
    wit_component::ComponentEncoder::default()
        .module(&core)
        .expect("an encoder")
        .validate(true)
        .encode()
        .expect("a component")
}

/// Everything a [`Prepared`] borrows, owned so the change can outlive the
/// expression that builds it.
struct Change {
    address: nerpa_core::address::Address,
    desired: std::collections::BTreeMap<String, Value>,
    secret: BTreeSet<String>,
    observed: Observed,
    target: Target,
}

impl Change {
    /// A creation for `guest:file.note@localhost` whose machine fact names the
    /// URL the provider should reach.
    fn with_url(url: &str) -> Self {
        let target = Target::new("localhost").expect("a target");
        let mut observed = Observed::new();
        observed.learn(target.clone(), "url", url);
        Self {
            address: address("guest:file.note@localhost"),
            desired: [(String::from("path"), Value::Text(String::from("/tmp/note")))]
                .into_iter()
                .collect(),
            secret: BTreeSet::new(),
            observed,
            target,
        }
    }

    fn prepared(&self) -> Prepared<'_> {
        Prepared {
            address: &self.address,
            kind: Sending::Create,
            roused: false,
            resent: false,
            desired: &self.desired,
            secret: &self.secret,
            recorded: None,
            facts: self.observed.facts_of(Some(&self.target)),
        }
    }
}

/// Loads a provider from a manifest that allows `hosts` and holds `token`.
fn load(scratch: &Scratch, token: &str, hosts: &str) -> Wasm {
    let manifest = scratch.join("provider.toml");
    fs::write(
        &manifest,
        format!(
            "name = \"guest\"\ntoken = {token:?}\nhosts = [{hosts}]\n\n[kind.file]\nimmutable = [\"path\"]\n"
        ),
    )
    .expect("a manifest");
    let component_path = scratch.join("provider.wasm");
    fs::write(&component_path, component()).expect("a component");
    Wasm::load(&manifest, &component_path).expect("a loaded provider")
}

#[test]
fn a_request_outside_the_manifest_is_refused_before_it_is_sent() {
    let scratch = Scratch::new("credential-refused");
    let wasm = load(&scratch, "secret-token", "\"api.example.com\"");

    let change = Change::with_url("https://evil.example.com/");
    let error = wasm.prepare(&change.prepared()).expect_err("a refusal");
    let problem = match error {
        ProviderError::World { problem, .. } => problem,
        other => panic!("expected a world failure: {other:?}"),
    };
    assert!(
        problem.contains("evil.example.com"),
        "the refused host is named: {problem}"
    );
}

#[test]
fn a_request_inside_the_manifest_carries_the_credential_the_host_holds() {
    // A local server standing in for the manifest's host, which records the
    // Authorization header the host attaches to a request the component only
    // ever handed over a handle for.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a listener");
    let port = listener.local_addr().expect("a port").port();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("a connection");
        let mut request = [0u8; 4096];
        let read = stream.read(&mut request).expect("a request");
        let request = String::from_utf8_lossy(&request[..read]).to_string();
        assert!(
            request.contains("Bearer secret-token"),
            "the host attaches the credential it alone holds: {request}"
        );
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .expect("a response");
    });

    let scratch = Scratch::new("credential-attached");
    let wasm = load(&scratch, "secret-token", "\"127.0.0.1\"");

    let change = Change::with_url(&format!("http://127.0.0.1:{port}/"));
    let planned = wasm.prepare(&change.prepared()).expect("a plan");
    let Some(nerpa_proto::Operation::File { content, .. }) = planned.operations.first() else {
        panic!("expected a file operation: {planned:?}");
    };
    assert_eq!(content, b"ok");

    server.join().expect("the server");
}
