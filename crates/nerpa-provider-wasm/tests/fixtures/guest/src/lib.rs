//! A file provider whose whole job is to create one fixed file, for the test
//! that proves the WIT world's operations are the ones that reach the machine.
//!
//! It also spends the credential the host holds on a request whose URL it reads
//! from the change's facts, so the credential test can prove the host restricts
//! the host and attaches the value the component never sees.

wit_bindgen::generate!({
    world: "provider",
    path: "../../../wit",
});

struct FileProvider;

impl Guest for FileProvider {
    fn read(_batch: Vec<Reading>, _facts: Facts) -> Result<ReadResult, String> {
        Ok(ReadResult {
            attributes: Vec::new(),
            premise: Vec::new(),
        })
    }

    fn prepare(change: Prepared) -> Result<Planned, String> {
        // A `url` in the change's facts means "spend the credential the host
        // holds on this request and make the answer the file's content".
        // Without one, the provider writes a fixed file. The component only
        // ever holds an opaque handle; the host restricts the host and attaches
        // the value itself.
        let content = match change.facts.iter().find(|(name, _)| name == "url") {
            Some((_, url)) => {
                let credential = credential_handle();
                let response = request(&credential, url)?;
                response.body
            }
            None => b"made by the wasm provider\n".to_vec(),
        };

        Ok(Planned {
            operations: vec![Operation::File(FileOperation {
                path: "/tmp/nerpa-wasm-created".to_owned(),
                content,
                mode: 0o644,
                secret: false,
            })],
            forget_after: None,
            carries: Vec::new(),
        })
    }

    fn conclude(_change: Concluding, _became: Vec<Answered>) -> Result<Concluded, String> {
        Ok(Concluded::Became(Vec::new()))
    }

    fn afterwards(_address: Address, _kind: Sending) -> Afterwards {
        Afterwards::Read
    }
}

export!(FileProvider);
