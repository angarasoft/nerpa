//! A file provider whose whole job is to create one fixed file, for the test
//! that proves the WIT world's operations are the ones that reach the machine.

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

    fn prepare(_change: Prepared) -> Result<Planned, String> {
        Ok(Planned {
            operations: vec![Operation::File(FileOperation {
                path: "/tmp/nerpa-wasm-created".to_owned(),
                content: b"made by the wasm provider\n".to_vec(),
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
