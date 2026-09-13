//! Builds a complete traffic report from a JSON Lines input stream.

mod aggregate;
mod ids;
mod record;

use std::io::{self, BufRead, Read};

use aggregate::ReportAccumulator;
use record::{RequestRecord, is_blank};

pub(crate) use aggregate::Report;

const REQUESTS_PER_BUCKET: u64 = 5;
const BUCKET_SECONDS: i64 = 10;
const MAX_INPUT_LINE_BYTES: usize = 1024 * 1024;

/// Builds the complete traffic report from a JSON Lines input stream.
///
/// The returned report preserves the documented deterministic ordering and
/// counts malformed and blank input according to the CLI contract.
pub(crate) fn build_report(mut input: impl BufRead) -> io::Result<Report> {
    let mut accumulator = ReportAccumulator::new();
    let mut line = Vec::new();

    loop {
        line.clear();
        let bytes_read = (&mut input)
            .take((MAX_INPUT_LINE_BYTES + 1) as u64)
            .read_until(b'\n', &mut line)?;
        if bytes_read == 0 {
            break;
        }

        accumulator.received_line();
        let ended_with_newline = line.last() == Some(&b'\n');
        if ended_with_newline {
            line.pop();
        }

        if line.len() > MAX_INPUT_LINE_BYTES {
            if !ended_with_newline {
                input.skip_until(b'\n')?;
            }
            accumulator.received_malformed_input();
        } else if is_blank(&line) {
            accumulator.received_blank_line();
        } else {
            match RequestRecord::parse(&line) {
                Some(record) => accumulator.add_request(record),
                None => accumulator.received_malformed_input(),
            }
        }
    }

    Ok(accumulator.finish())
}
