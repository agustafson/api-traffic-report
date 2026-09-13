use std::{
    fs,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::{Value, json};

static TEMP_FILE_SEQUENCE: AtomicUsize = AtomicUsize::new(0);
const MAX_INPUT_LINE_BYTES: usize = 1024 * 1024;

fn write_log(contents: &str) -> std::path::PathBuf {
    let unique = format!(
        "api-traffic-report-cli-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos(),
        TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed),
    );
    let path = std::env::temp_dir().join(unique);
    fs::write(&path, contents).expect("write temporary JSON Lines input");
    path
}

fn run_report(input: &str) -> Output {
    let path = write_log(input);
    let output = Command::new(env!("CARGO_BIN_EXE_api-traffic-report"))
        .arg(&path)
        .output()
        .expect("run the compiled report command");
    fs::remove_file(path).expect("remove temporary JSON Lines input");
    output
}

fn assert_failed_without_report(output: Output) {
    assert!(!output.status.success(), "command unexpectedly succeeded");
    assert!(output.stdout.is_empty(), "stdout: {:?}", output.stdout);
    assert!(
        !output.stderr.is_empty(),
        "a failed command must write a diagnostic to stderr"
    );
}

fn assert_report(output: Output, expected: Value) {
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).expect("stdout is a single JSON report"),
        expected,
    );
}

#[test]
fn reports_valid_traffic_and_a_client_wide_rate_limit_violation() {
    let input = concat!(
        r#"{"request_id":"a1_1","timestamp":"2024-01-15T10:00:00Z","client_id":"acct_1","endpoint":"/v1/widgets","status_code":200}"#,
        "\n",
        r#"{"request_id":"a1_2","timestamp":"2024-01-15T10:00:02Z","client_id":"acct_1","endpoint":"/v1/widgets","status_code":200}"#,
        "\n",
        r#"{"request_id":"a1_3","timestamp":"2024-01-15T10:00:04Z","client_id":"acct_1","endpoint":"/v1/widgets","status_code":200}"#,
        "\n",
        r#"{"request_id":"a1_4","timestamp":"2024-01-15T10:00:05Z","client_id":"acct_1","endpoint":"/v1/widgets","status_code":200}"#,
        "\n",
        r#"{"request_id":"a1_5","timestamp":"2024-01-15T10:00:06Z","client_id":"acct_1","endpoint":"/v1/widgets","status_code":200}"#,
        "\n",
        r#"{"request_id":"a1_6","timestamp":"2024-01-15T10:00:08Z","client_id":"acct_1","endpoint":"/v1/widgets","status_code":200}"#,
        "\n",
    );
    let output = run_report(input);

    assert_report(
        output,
        json!({
            "total_line_count": 6,
            "processed_line_count": 6,
            "valid_request_count": 6,
            "malformed_input_count": 0,
            "ignored_blank_line_count": 0,
            "client_bucket_rate_limit_violation_count": 1,
            "rate_limit_excess_request_count": 1,
            "rate_limit_violating_clients": ["acct_1"],
            "request_counts_by_client_endpoint_status": [{
                "client_id": "acct_1",
                "endpoint": "/v1/widgets",
                "status_code": 200,
                "request_count": 6,
            }],
            "rate_limit_counts_by_client": [{
                "client_id": "acct_1",
                "client_bucket_rate_limit_violation_count": 1,
                "rate_limit_excess_request_count": 1,
            }],
        }),
    );
}

#[test]
fn aligns_offset_timestamps_to_utc_buckets_and_orders_report_arrays() {
    let input = concat!(
        r#"{"request_id":"b1","timestamp":"2024-01-15T11:00:08+01:00","client_id":"bravo","endpoint":"/v1/zulu","status_code":200}"#,
        "\n",
        r#"{"request_id":"b2","timestamp":"2024-01-15T10:00:07Z","client_id":"bravo","endpoint":"/v1/alpha","status_code":204}"#,
        "\n",
        r#"{"request_id":"b3","timestamp":"2024-01-15T11:00:06+01:00","client_id":"bravo","endpoint":"/v1/zulu","status_code":200}"#,
        "\n",
        r#"{"request_id":"b4","timestamp":"2024-01-15T10:00:05Z","client_id":"bravo","endpoint":"/v1/alpha","status_code":204}"#,
        "\n",
        r#"{"request_id":"b5","timestamp":"2024-01-15T11:00:04+01:00","client_id":"bravo","endpoint":"/v1/zulu","status_code":200}"#,
        "\n",
        r#"{"request_id":"b6","timestamp":"2024-01-15T10:00:03Z","client_id":"bravo","endpoint":"/v1/alpha","status_code":204}"#,
        "\n",
        r#"{"request_id":"a1","timestamp":"2024-01-15T10:00:09Z","client_id":"alpha","endpoint":"/v1/beta","status_code":200}"#,
        "\n",
        r#"{"request_id":"a2","timestamp":"2024-01-15T11:00:08+01:00","client_id":"alpha","endpoint":"/v1/alpha","status_code":500}"#,
        "\n",
        r#"{"request_id":"a3","timestamp":"2024-01-15T10:00:07Z","client_id":"alpha","endpoint":"/v1/beta","status_code":200}"#,
        "\n",
        r#"{"request_id":"a4","timestamp":"2024-01-15T11:00:06+01:00","client_id":"alpha","endpoint":"/v1/alpha","status_code":500}"#,
        "\n",
        r#"{"request_id":"a5","timestamp":"2024-01-15T10:00:05Z","client_id":"alpha","endpoint":"/v1/beta","status_code":200}"#,
        "\n",
        r#"{"request_id":"a6","timestamp":"2024-01-15T11:00:04+01:00","client_id":"alpha","endpoint":"/v1/alpha","status_code":500}"#,
        "\n",
        r#"{"request_id":"a7","timestamp":"2024-01-15T10:00:10Z","client_id":"alpha","endpoint":"/v1/alpha","status_code":500}"#,
        "\n",
    );

    let output = run_report(input);

    assert_report(
        output,
        json!({
            "total_line_count": 13,
            "processed_line_count": 13,
            "valid_request_count": 13,
            "malformed_input_count": 0,
            "ignored_blank_line_count": 0,
            "client_bucket_rate_limit_violation_count": 2,
            "rate_limit_excess_request_count": 2,
            "rate_limit_violating_clients": ["alpha", "bravo"],
            "request_counts_by_client_endpoint_status": [
                {"client_id": "alpha", "endpoint": "/v1/alpha", "status_code": 500, "request_count": 4},
                {"client_id": "alpha", "endpoint": "/v1/beta", "status_code": 200, "request_count": 3},
                {"client_id": "bravo", "endpoint": "/v1/alpha", "status_code": 204, "request_count": 3},
                {"client_id": "bravo", "endpoint": "/v1/zulu", "status_code": 200, "request_count": 3},
            ],
            "rate_limit_counts_by_client": [
                {
                    "client_id": "alpha",
                    "client_bucket_rate_limit_violation_count": 1,
                    "rate_limit_excess_request_count": 1,
                },
                {
                    "client_id": "bravo",
                    "client_bucket_rate_limit_violation_count": 1,
                    "rate_limit_excess_request_count": 1,
                },
            ],
        }),
    );
}

#[test]
fn counts_blank_and_malformed_lines_without_losing_valid_records() {
    let input = concat!(
        r#"{"request_id":"valid_1","timestamp":"2024-01-15T10:00:00Z","client_id":"acct_1","endpoint":"/v1/widgets","status_code":200}"#,
        "\n",
        " \t\r\n",
        "[]\n",
        r#"{"request_id":"missing_endpoint","timestamp":"2024-01-15T10:00:01Z","client_id":"acct_1","status_code":200}"#,
        "\n",
        r#"{"request_id":"wrong_type","timestamp":"2024-01-15T10:00:02Z","client_id":1,"endpoint":"/v1/widgets","status_code":200}"#,
        "\n",
        r#"{"request_id":"empty_endpoint","timestamp":"2024-01-15T10:00:03Z","client_id":"acct_1","endpoint":"","status_code":200}"#,
        "\n",
        r#"{"request_id":"bad_timestamp","timestamp":"not-a-timestamp","client_id":"acct_1","endpoint":"/v1/widgets","status_code":200}"#,
        "\n",
        r#"{"request_id":"bad_status","timestamp":"2024-01-15T10:00:04Z","client_id":"acct_1","endpoint":"/v1/widgets","status_code":99}"#,
        "\n",
        r#"{"request_id":"valid_2","timestamp":"2024-01-15T10:00:05Z","client_id":"acct_1","endpoint":"/v1/widgets","status_code":200,"upstream_provider":"example"}"#,
        "\n",
        "\n",
    );

    let output = run_report(input);

    assert_report(
        output,
        json!({
            "total_line_count": 10,
            "processed_line_count": 8,
            "valid_request_count": 2,
            "malformed_input_count": 6,
            "ignored_blank_line_count": 2,
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

#[test]
fn discards_oversized_lines_and_resumes_at_the_next_record() {
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

    let output = run_report(&input);

    assert_report(
        output,
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

#[test]
fn reports_an_entirely_malformed_file_with_no_traffic_summaries() {
    let input = concat!(
        r#"{"request_id":"","timestamp":"2024-01-15T10:00:00Z","client_id":"acct_1","endpoint":"/v1/widgets","status_code":200}"#,
        "\n",
    );

    let output = run_report(input);

    assert_report(
        output,
        json!({
            "total_line_count": 1,
            "processed_line_count": 1,
            "valid_request_count": 0,
            "malformed_input_count": 1,
            "ignored_blank_line_count": 0,
            "client_bucket_rate_limit_violation_count": 0,
            "rate_limit_excess_request_count": 0,
            "rate_limit_violating_clients": [],
            "request_counts_by_client_endpoint_status": [],
            "rate_limit_counts_by_client": [],
        }),
    );
}

#[test]
fn accepts_status_code_599_and_rejects_status_code_600() {
    let input = concat!(
        r#"{"request_id":"valid_599","timestamp":"2024-01-15T10:00:00Z","client_id":"acct_1","endpoint":"/v1/widgets","status_code":599}"#,
        "\n",
        r#"{"request_id":"invalid_600","timestamp":"2024-01-15T10:00:01Z","client_id":"acct_1","endpoint":"/v1/widgets","status_code":600}"#,
        "\n",
    );

    let output = run_report(input);

    assert_report(
        output,
        json!({
            "total_line_count": 2,
            "processed_line_count": 2,
            "valid_request_count": 1,
            "malformed_input_count": 1,
            "ignored_blank_line_count": 0,
            "client_bucket_rate_limit_violation_count": 0,
            "rate_limit_excess_request_count": 0,
            "rate_limit_violating_clients": [],
            "request_counts_by_client_endpoint_status": [{
                "client_id": "acct_1",
                "endpoint": "/v1/widgets",
                "status_code": 599,
                "request_count": 1,
            }],
            "rate_limit_counts_by_client": [],
        }),
    );
}

#[test]
fn rejects_missing_and_extra_log_file_arguments_without_a_report() {
    let missing_argument = Command::new(env!("CARGO_BIN_EXE_api-traffic-report"))
        .output()
        .expect("run the compiled report command without an argument");
    assert_failed_without_report(missing_argument);

    let extra_argument = Command::new(env!("CARGO_BIN_EXE_api-traffic-report"))
        .args(["first.jsonl", "second.jsonl"])
        .output()
        .expect("run the compiled report command with extra arguments");
    assert_failed_without_report(extra_argument);
}

#[test]
fn reports_an_unreadable_log_file_only_on_stderr() {
    let path = write_log("");
    fs::remove_file(&path).expect("make the temporary log path unreadable");

    let output = Command::new(env!("CARGO_BIN_EXE_api-traffic-report"))
        .arg(path)
        .output()
        .expect("run the compiled report command with an absent log file");

    assert_failed_without_report(output);
}

#[test]
fn aggregates_a_deterministic_larger_input() {
    let mut input = String::new();
    for (client_id, endpoint, status_code, requests_per_bucket) in [
        ("charlie", "/v1/charlie", 202, 8),
        ("bravo", "/v1/bravo", 201, 5),
        ("alpha", "/v1/alpha", 200, 6),
    ] {
        for bucket in (0..12).rev() {
            let seconds_since_hour = bucket * 10;
            let minute = seconds_since_hour / 60;
            let second = seconds_since_hour % 60;
            for request in 0..requests_per_bucket {
                input.push_str(&format!(
                    "{{\"request_id\":\"{client_id}_{bucket}_{request}\",\"timestamp\":\"2024-01-15T10:{minute:02}:{second:02}Z\",\"client_id\":\"{client_id}\",\"endpoint\":\"{endpoint}\",\"status_code\":{status_code}}}\n",
                ));
            }
        }
    }

    let output = run_report(&input);

    assert_report(
        output,
        json!({
            "total_line_count": 228,
            "processed_line_count": 228,
            "valid_request_count": 228,
            "malformed_input_count": 0,
            "ignored_blank_line_count": 0,
            "client_bucket_rate_limit_violation_count": 24,
            "rate_limit_excess_request_count": 48,
            "rate_limit_violating_clients": ["alpha", "charlie"],
            "request_counts_by_client_endpoint_status": [
                {"client_id": "alpha", "endpoint": "/v1/alpha", "status_code": 200, "request_count": 72},
                {"client_id": "bravo", "endpoint": "/v1/bravo", "status_code": 201, "request_count": 60},
                {"client_id": "charlie", "endpoint": "/v1/charlie", "status_code": 202, "request_count": 96},
            ],
            "rate_limit_counts_by_client": [
                {
                    "client_id": "alpha",
                    "client_bucket_rate_limit_violation_count": 12,
                    "rate_limit_excess_request_count": 12,
                },
                {
                    "client_id": "charlie",
                    "client_bucket_rate_limit_violation_count": 12,
                    "rate_limit_excess_request_count": 36,
                },
            ],
        }),
    );
}

#[test]
fn sanity_checks_against_the_supplied_example_input() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/sample_input/requests.jsonl");
    let output = Command::new(env!("CARGO_BIN_EXE_api-traffic-report"))
        .arg(path)
        .output()
        .expect("run the compiled report command against the supplied example input");

    assert_report(
        output,
        json!({
            "total_line_count": 8,
            "processed_line_count": 8,
            "valid_request_count": 8,
            "malformed_input_count": 0,
            "ignored_blank_line_count": 0,
            "client_bucket_rate_limit_violation_count": 1,
            "rate_limit_excess_request_count": 1,
            "rate_limit_violating_clients": ["acct_1"],
            "request_counts_by_client_endpoint_status": [
                {"client_id": "acct_1", "endpoint": "/v1/widgets", "status_code": 200, "request_count": 6},
                {"client_id": "acct_2", "endpoint": "/v1/reports", "status_code": 200, "request_count": 2},
            ],
            "rate_limit_counts_by_client": [{
                "client_id": "acct_1",
                "client_bucket_rate_limit_violation_count": 1,
                "rate_limit_excess_request_count": 1,
            }],
        }),
    );
}

#[test]
fn counts_identical_valid_records_independently() {
    let repeated_record = r#"{"request_id":"repeat_1","timestamp":"2024-01-15T10:00:00Z","client_id":"acct_1","endpoint":"/v1/widgets","status_code":200}"#;
    let input = format!("{repeated_record}\n{repeated_record}\n");

    let output = run_report(&input);

    assert_report(
        output,
        json!({
            "total_line_count": 2,
            "processed_line_count": 2,
            "valid_request_count": 2,
            "malformed_input_count": 0,
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
