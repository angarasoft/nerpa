//! Bytes in, bytes out.
//!
//! Reads requests from standard input and writes answers to standard output,
//! one frame at a time, until the other end goes away. It knows nothing about
//! how it was reached — see [`nerpa_executor`] and decision 0032 — so `ssh`, a
//! package already on the machine, and anything else that can hand it a pipe are
//! the same thing from in here.

use std::io::{self, Write as _};
use std::process::ExitCode;

use nerpa_proto::Response;

fn main() -> ExitCode {
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();

    loop {
        let request = match nerpa_proto::read_request(&mut input) {
            Ok(request) => request,
            // The engine hung up. That is how a run ends, not how it fails.
            //
            // A pipe that broke mid-prefix looks the same from here, because
            // that is all the wire distinguishes. Saying so rather than
            // pretending otherwise: a truncated *frame* is reported, a truncated
            // length is not.
            Err(nerpa_proto::WireError::Connection(_)) => return ExitCode::SUCCESS,
            Err(problem) => {
                let _ = writeln!(io::stderr(), "nerpa-executor: {problem}");
                return ExitCode::FAILURE;
            }
        };

        let response = Response::new(nerpa_executor::answer(request.body()));
        if let Err(problem) = nerpa_proto::write(&mut output, &response) {
            let _ = writeln!(io::stderr(), "nerpa-executor: {problem}");
            return ExitCode::FAILURE;
        }
        if output.flush().is_err() {
            return ExitCode::FAILURE;
        }
    }
}
