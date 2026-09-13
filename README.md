# API Traffic Report

A command-line program that reads one JSON Lines API request log and writes one JSON traffic report to standard output.

## Run

### Prerequisites

Building and running this program requires the Rust toolchain, which includes `cargo`. If you don't already have it installed:

**macOS or Linux**

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Follow the prompts, then restart your terminal (or run `source "$HOME/.cargo/env"`) so `cargo` is on your `PATH`.

**Windows**

Download and run [`rustup-init.exe`](https://win.rustup.rs) from the official Rust site, or, with `winget`:

```powershell
winget install Rustlang.Rustup
```

Follow the prompts, then open a new terminal so `cargo` is on your `PATH`.

**Rust version**

The project requires Rust **1.85 or newer** (edition 2024) and is tested with Rust 1.85.1 and 1.98.1. Any current stable toolchain works. To use the minimum supported version explicitly:

```sh
rustup toolchain install 1.85
```

and prefix the `cargo` commands below with `cargo +1.85`.

### Build and run

Build an optimized binary once:

```sh
cargo build --release
```

Then run it against a log file:

```sh
./target/release/api-traffic-report path/to/requests.jsonl
```

On Windows the binary is `.\target\release\api-traffic-report.exe`.

The command accepts exactly one positional file path. It writes exactly one JSON document followed by a newline to standard output for every readable file. Invalid command lines and unreadable files write a diagnostic to standard error, exit non-zero, and do not write a report.

### Test

```sh
cargo test
```

## End-to-end design

- The command opens the supplied file and reads it one line at a time.
- Blank lines are counted and ignored; each nonblank line of at most 4 KiB is parsed as a JSON object and validated as a request record. Oversized lines are drained without being retained and counted as malformed.
- Valid records update two in-memory aggregates: a traffic cube keyed by `(client_id, endpoint, status_code)` and a fixed UTC ten-second bucket count keyed by `(client_id, bucket)`.
- After the input ends, the bucket counts are reduced into rate-limit summaries and all aggregates are emitted as the report.

This keeps the public interface deliberately small: the input file path, process exit status, standard output JSON, and standard error diagnostics are the complete CLI contract.

## Rate-limit rules

- One globally configured policy applies to every client: at most five requests per fixed, UTC-aligned ten-second bucket. A bucket includes its start and excludes its end, so `10:00:00` up to but excluding `10:00:10` is one bucket.
- A bucket with more than five requests is one violation, and each request above five in it is an excess request.
- A client is evaluated across every endpoint and provider.
- Version 1 does not need command-line configuration or a configuration file.

### Why this rule

- **Five requests per ten seconds:** calibrated so the supplied sample flags `acct_1` (six requests in eight seconds) and not `acct_2`.
- **Client-wide:** `client_id` is globally consistent across upstream providers, so a client-wide policy is meaningful even when a client's requests are logged by different providers, and it catches bursts spread across endpoints.
- **Fixed buckets:** simple, deterministic, and directly explainable in a report. Bucket counts do not depend on input order, which matters because third-party logs may be out of order. Counts for the same `(client_id, bucket)` can be summed across files or workers before the limit is evaluated. Only one counter per active client bucket is retained, not per-request timestamps.

### Tradeoff: bursts across a bucket boundary

Fixed buckets reset at each boundary, so a client can send up to twice the limit in a short span that straddles one. Five requests at `10:00:09.9` and five at `10:00:10.0` are ten requests in 100 ms, yet neither bucket exceeds five, so no violation is reported. A test pins this behaviour.

The alternatives close that gap at a cost:

- **Sliding window:** flags any ten-second span containing more than five requests. It is exact, but it needs each client's requests in timestamp order, which for unordered input means holding and sorting their timestamps.
- **Token bucket:** each client holds up to five tokens that refill at 0.5 per second, and a request that finds no token is a violation. It permits short bursts while enforcing a steady rate with only one small state per client, but it also needs each client's requests in timestamp order, and partial results from separate workers cannot simply be summed.

For an offline report over unordered third-party logs, fixed buckets were chosen for order independence and mergeability, accepting the boundary blind spot.

## JSON report specification

The report is one compact JSON object with fields emitted in this order:

| Field | Type | Meaning |
|---|---|---|
| `total_line_count` | integer | Physical lines in the file. A single final line terminator does not add a line. |
| `processed_line_count` | integer | Nonblank lines examined as request records. Equals `valid_request_count + malformed_input_count`. |
| `valid_request_count` | integer | Lines that produced a valid request record. |
| `malformed_input_count` | integer | Nonblank lines discarded as malformed. |
| `ignored_blank_line_count` | integer | Empty or whitespace-only lines, which are ignored rather than counted as malformed. |
| `client_bucket_rate_limit_violation_count` | integer | Client buckets, across all clients, with more than five requests. |
| `rate_limit_excess_request_count` | integer | Requests above five, summed over all violating client buckets. |
| `rate_limit_violating_clients` | array of strings | Clients with at least one violating bucket. |
| `request_counts_by_client_endpoint_status` | array of objects | One `{client_id, endpoint, status_code, request_count}` row per distinct combination of valid requests. |
| `rate_limit_counts_by_client` | array of objects | One `{client_id, client_bucket_rate_limit_violation_count, rate_limit_excess_request_count}` row per violating client. |

All count fields are nonnegative integers. The arrays are deterministic: `rate_limit_violating_clients` and `rate_limit_counts_by_client` are sorted by ascending `client_id`, and `request_counts_by_client_endpoint_status` is sorted by ascending `(client_id, endpoint, status_code)`.

`rate_limit_violating_clients` lists the same clients as `rate_limit_counts_by_client`. It is kept as a direct answer to "which clients violated the rate limit?", so consumers that only need to alert on or look up offending clients do not have to project the detailed rows.

For the supplied `sample_input/requests.jsonl`, the program writes the following report (pretty-printed here; the program emits it on one line):

```json
{
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
    {"client_id": "acct_2", "endpoint": "/v1/reports", "status_code": 200, "request_count": 2}
  ],
  "rate_limit_counts_by_client": [
    {"client_id": "acct_1", "client_bucket_rate_limit_violation_count": 1, "rate_limit_excess_request_count": 1}
  ]
}
```

## Assumptions and decisions

This is the active decision log for the exercise. It records deliberate interpretations of requirements that the brief leaves open.

- **Input contract:** The program accepts one required positional log-file path. Standard input is not an input mode. File and command-line errors are reported on standard error with a non-zero exit status.
- **Client identity:** `client_id` is globally consistent across all upstream providers. A client rate limit therefore covers that client's traffic across every endpoint and provider.
- **Request ID scope**: `request_id` is not assumed globally unique across upstream providers; it may be unique only within a provider. Because the input has no provider identifier, v1 treats `request_id` as opaque and does not deduplicate records.
- **Rate-limit reporting:** The report distinguishes client-time-bucket violations from excess requests. A violating client bucket is counted once when its request count exceeds five; its excess request count is the amount above five. Neither metric claims that an upstream request was blocked.
- **Traffic aggregation:** Request counts are grouped by the exact `client_id`, `endpoint`, and `status_code` combination. The traffic cube contains only those grouping keys and `request_count`; report consumers can roll its rows up to client, endpoint, or status-code views as needed. Client-wide rate-limit metrics remain in a separate client summary because the policy covers all client endpoints together.
- **Malformed input:** A nonblank line is malformed and ignored when it exceeds 4 KiB (4,096 bytes, excluding its LF terminator), is not a JSON object, omits a required field, gives a field the wrong type, gives a required string that is empty or whitespace-only, contains an invalid RFC 3339 timestamp, or gives a status code outside 100 through 599. The line limit leaves several times the expected space for request IDs, client IDs, and endpoints of up to roughly 256 characters each. Unknown extra fields are accepted within that limit. The endpoint needs no syntax validation beyond being non-blank.
- **Blank required strings:** An empty or whitespace-only required string is treated as missing, because it cannot identify a request, client, endpoint, or time. Non-blank values are kept verbatim rather than trimmed: identifiers come from third-party providers, and silently normalizing `" acct_1 "` to `"acct_1"` could merge traffic that the source considers distinct.
- **Blank input:** Blank or whitespace-only lines are ignored rather than classified as malformed. The report distinguishes the number of physical lines, nonblank processed lines, and ignored blank lines. A single final line terminator does not make an extra blank line; an additional empty line does.
- **Duplicate records:** Version 1 counts every valid log record, including duplicate records. Source-aware idempotency or duplicate detection is a future improvement, because duplicate data may otherwise be indistinguishable from a legitimate retry.
- **Output and failures:** A readable input file always produces exactly one JSON report on standard output, even when it contains malformed records. Command-line or file-access failures write a diagnostic to standard error, exit non-zero, and produce no report.
- **Aggregation resources:** Version 1 streams the file through a reusable current-line buffer capped at 4 KiB plus one byte for overflow detection, and holds aggregate counts and summaries in memory. Oversized lines are drained through their line ending without being retained. Aggregate memory grows with the number of distinct traffic groups and client-time buckets, rather than with the raw input content. Unbounded-cardinality scaling is future work.

## Implementation notes

- JSON parsing and serialization use `serde_json`; RFC 3339 timestamps are parsed with `time` and converted to Unix seconds before fixed-bucket calculation.
- The report uses ordered maps for its aggregate keys, which supplies the documented deterministic array ordering without a second sort pass.
- A record must have non-blank string `request_id`, `timestamp`, `client_id`, and `endpoint` fields, plus an integral status code from 100 through 599. Extra object fields are retained by neither the validation nor the report.
- The implementation deliberately counts valid duplicate lines independently. It reports observed input rather than claiming an upstream system accepted or rejected any particular request.

## Future improvements

- Support client-specific rate-limit policies.
- Offer a sliding-window or token-bucket policy where catching bursts across bucket boundaries matters more than order independence.
- Add source-aware idempotency or duplicate detection.
- Support bounded-memory aggregation for exceptionally high-cardinality clients and traffic groups, such as an external store or sorted spill files: a file with millions of distinct (client, endpoint, status) groups or (client, UTC bucket) pairs creates millions of map entries, potentially approaching raw-file memory use.
- Externalise configuration such as parameters for rate-limiting.
- Extend the deterministic end-to-end coverage with additional reviewable fixtures for every malformed field.
- Add `AGENTS.md`, `CLAUDE.md` & `CODING_STANDARDS.md` to store AI & huma guidance for the codebase. 
- Parallel file processing: Split JSONL only on line boundaries, let workers build local aggregates, then merge. For the current fixed-bucket policy, merging (client_id, bucket) counts before evaluating the limit is correct. Sharding by client_id is even better: every client’s rate state lands on one worker.
- Persistence: if reports need to be generated later or over a continuous stream. Store either raw normalized request events, aggregates, or both. Raw events preserve flexibility for new analyses; bucketed aggregates cost less but cannot answer arbitrary new questions later.

## Verification approach

- Cover the public command-line contract with simple end-to-end integration tests, run with `cargo test`.
- Keep named JSON Lines fixtures small and reviewable, including the supplied example and boundary or malformed-input cases such as CRLF line endings, a byte order mark, fractional and lowercase timestamps, bursts across a bucket boundary, and blank required strings.
- Generate a larger deterministic data set within a test to exercise aggregation at greater volume.
- Do not add fuzz testing in this exercise.

## AI assistance disclosure

- Codex/Claude/AI assisted with requirement analysis, paired-TDD coordination, and implementation. The work remains subject to user review.
- Used a custom [agustafson/skills/tdd-pair](https://github.com/agustafson/skills/tree/main/tdd-pair) skill which uses separate agents to write the tests and implementation. 
