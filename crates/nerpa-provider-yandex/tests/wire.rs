//! The wire itself: what the HTTP client sends is what one local listener
//! sees, header by header.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nerpa_provider_yandex::{Api, Http};

#[test]
fn a_caller_content_type_is_not_duplicated() {
    // The JSON verbs label a body that claims nothing; a caller that names
    // its own Content-Type sends exactly that one, not two.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let seen: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let recorder = {
        let seen = Arc::clone(&seen);
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut head = Vec::new();
            let mut byte = [0u8; 1];
            loop {
                match stream.read(&mut byte) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        head.push(byte[0]);
                        if head.ends_with(b"\r\n\r\n") {
                            break;
                        }
                    }
                }
            }
            *seen.lock().unwrap() = String::from_utf8_lossy(&head).into_owned();
            let answer = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}";
            stream.write_all(answer.as_bytes()).unwrap();
        })
    };
    let wire = Http::new(Duration::from_secs(10));
    let answer = wire
        .post(
            &format!("http://{address}/anything"),
            &[("Content-Type", "text/plain")],
            "a body",
        )
        .expect("the listener answers");
    assert_eq!(answer.status, 200);
    recorder.join().unwrap();
    let head = seen.lock().unwrap().clone();
    let labels = head
        .lines()
        .filter(|line| line.to_ascii_lowercase().starts_with("content-type:"))
        .count();
    assert_eq!(labels, 1, "one Content-Type, the caller's: {head}");
    assert!(
        head.to_ascii_lowercase()
            .contains("content-type: text/plain"),
        "the caller's label is the one that is sent: {head}"
    );
}
