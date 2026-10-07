//! A file provider whose whole job is to create the file the change declares,
//! for the tests that prove the WIT world's operations reach the machine and
//! are applied there.
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
        // The path the change settles, carried as CBOR in the desired
        // attributes. The provider names the resource where it is declared, so
        // the apply creates exactly what the configuration asked for.
        let path = change
            .desired
            .iter()
            .find(|(name, _)| name == "path")
            .map(|(_, value)| cbor_text(value))
            .ok_or_else(|| "the change has no `path` attribute".to_owned())?
            .ok_or_else(|| "the `path` attribute is not text".to_owned())?;

        // A `url` in the change's facts means "spend the credential the host
        // holds on this request and make the answer the file's content".
        // Without one, the provider writes a fixed content. The component only
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
                path,
                content,
                mode: 0o644,
                secret: false,
            })],
            forget_after: None,
            carries: Vec::new(),
        })
    }

    fn conclude(change: Concluding, _became: Vec<Answered>) -> Result<Concluded, String> {
        // What the change declared is what the write left behind: the path the
        // provider prepared and the executor wrote. Reporting it is what lets a
        // later run see the resource where it was made.
        Ok(Concluded::Became(change.plain))
    }

    fn afterwards(_address: Address, _kind: Sending) -> Afterwards {
        Afterwards::Read
    }
}

export!(FileProvider);

/// Decodes a CBOR text string, which is how a settled path travels. Enough for
/// the one attribute this provider reads; the full value grammar lives in the
/// host's `nerpa_core::Value`.
fn cbor_text(bytes: &[u8]) -> Option<String> {
    let (head, rest) = bytes.split_first()?;
    let (length, rest) = match *head {
        // A definite-length text string shorter than 24.
        0x60..=0x77 => (usize::from(*head & 0x1f), rest),
        // One byte of length follows.
        0x78 => (usize::from(*rest.first()?), &rest[1..]),
        // Two bytes of length follow, big-endian.
        0x79 => {
            let length = u16::from_be_bytes([rest.first().copied()?, rest.get(1).copied()?]);
            (usize::from(length), &rest[2..])
        }
        _ => return None,
    };
    let text = rest.get(..length)?;
    std::str::from_utf8(text).ok().map(str::to_owned)
}
