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
const DEFAULT_MAX_INPUT_LINE_BYTES: usize = 4 * 1024;

/// Builds the complete traffic report from a JSON Lines input stream.
///
/// The returned report preserves the documented deterministic ordering and
/// counts malformed and blank input according to the CLI contract.
pub(crate) fn build_report(input: impl BufRead) -> io::Result<Report> {
    build_report_with_max_line_bytes(input, DEFAULT_MAX_INPUT_LINE_BYTES)
}

fn build_report_with_max_line_bytes(
    mut input: impl BufRead,
    max_input_line_bytes: usize,
) -> io::Result<Report> {
    let mut accumulator = ReportAccumulator::new();
    let mut line = Vec::new();

    loop {
        line.clear();
        let bytes_read = (&mut input)
            .take((max_input_line_bytes + 1) as u64)
            .read_until(b'\n', &mut line)?;
        if bytes_read == 0 {
            break;
        }

        accumulator.received_line();
        let ended_with_newline = line.last() == Some(&b'\n');
        if ended_with_newline {
            line.pop();
        }

        if line.len() > max_input_line_bytes {
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::build_report_with_max_line_bytes;

    #[test]
    fn discards_oversized_lines_and_resumes_at_the_next_record() {
        const MAX_INPUT_LINE_BYTES: usize = 256;

        let prefix = r#"{"request_id":"at_limit","timestamp":"2024-01-15T10:00:00Z","client_id":"acct_1","endpoint":"/v1/widgets","status_code":200,"padding":""#;
        let suffix = r#""}"#;
        let at_limit = format!(
            "{prefix}{}{suffix}",
            "x".repeat(MAX_INPUT_LINE_BYTES - prefix.len() - suffix.len()),
        );
        assert_eq!(at_limit.len(), MAX_INPUT_LINE_BYTES);

        let oversized = format!("{at_limit} ");
        let following = r#"{"request_id":"following","timestamp":"2024-01-15T10:00:01Z","client_id":"acct_1","endpoint":"/v1/widgets","status_code":200}"#;
        let input = format!("{at_limit}\n{oversized}\n{following}\n");

        let report = build_report_with_max_line_bytes(input.as_bytes(), MAX_INPUT_LINE_BYTES)
            .expect("build report from bounded input");

        assert_eq!(
            serde_json::to_value(report).expect("serialize report"),
            json!({
                "total_line_count": 3,
                "processed_line_count": 3,
                "valid_request_count": 2,
                "malformed_input_count": 1,
                "ignored_blank_line_count": 0,
                "client_bucket_rate_limit_violation_count": 0,
                "rate_limit_excess_request_count": 0,
                "rate_limit_violating_clients": [],
                "request_counts_by_client_endpoint_status": [{
                    "client_id": "acct_1",
                    "endpoint": "/v1/widgets",
                    "status_code": 200,
                    "request_count": 2,
                }],
                "rate_limit_counts_by_client": [],
            }),
        );
    }
}
