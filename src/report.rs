//! Builds a complete traffic report from a JSON Lines input stream.

mod aggregate;
mod ids;
mod record;

use std::io::{self, BufRead};

use aggregate::ReportAccumulator;
use record::{RequestRecord, is_blank};

pub(crate) use aggregate::Report;

const REQUESTS_PER_BUCKET: u64 = 5;
const BUCKET_SECONDS: i64 = 10;

/// Builds the complete traffic report from a JSON Lines input stream.
///
/// The returned report preserves the documented deterministic ordering and
/// counts malformed and blank input according to the CLI contract.
pub(crate) fn build_report(mut input: impl BufRead) -> io::Result<Report> {
    let mut accumulator = ReportAccumulator::new();
    let mut line = Vec::new();

    while input.read_until(b'\n', &mut line)? != 0 {
        accumulator.received_line();
        if line.last() == Some(&b'\n') {
            line.pop();
        }

        if is_blank(&line) {
            accumulator.received_blank_line();
        } else {
            match RequestRecord::parse(&line) {
                Some(record) => accumulator.add_request(record),
                None => accumulator.received_malformed_input(),
            }
        }

        line.clear();
    }

    Ok(accumulator.finish())
}
