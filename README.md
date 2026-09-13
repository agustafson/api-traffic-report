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

**Any OS, once `rustup` is installed**

This project pins an exact toolchain version. Install it with:

```sh
rustup toolchain install 1.98.1
```

### Build and run

With that toolchain installed, run:

```sh
cargo +1.98.1 --quiet run -- path/to/requests.jsonl
```

The command accepts exactly one positional file path. It writes exactly one JSON document followed by a newline to standard output for every readable file. Invalid command lines and unreadable files write a diagnostic to standard error, exit non-zero, and do not write a report.

## End-to-end design

- The command opens the supplied file and reads it one line at a time.
- Blank lines are counted and ignored; each nonblank line of at most 4 KiB is parsed as a JSON object and validated as a request record. Oversized lines are drained without being retained and counted as malformed.
- Valid records update two in-memory aggregates: a traffic cube keyed by `(client_id, endpoint, status_code)` and a fixed UTC ten-second bucket count keyed by `(client_id, bucket)`.
- After the input ends, the bucket counts are reduced into rate-limit summaries and all aggregates are emitted as the report.

This keeps the public interface deliberately small: the input file path, process exit status, standard output JSON, and standard error diagnostics are the complete CLI contract.

## Rate-limit rules
- One globally configured policy applies to every client.
- Version 1 does not need command-line configuration or a configuration file.
- The limit is five requests per fixed, UTC-aligned ten-second bucket, with a bucket start included and its end excluded. Calibrated at this level so the sample flags `acct_1`.
- A client is evaluated across every endpoint and provider.

## JSON report schema

The report is one compact JSON object with fields emitted in this order:

```json
{
  "total_line_count": 0,
  "processed_line_count": 0,
  "valid_request_count": 0,
  "malformed_input_count": 0,
  "ignored_blank_line_count": 0,
  "client_bucket_rate_limit_violation_count": 0,
  "rate_limit_excess_request_count": 0,
  "rate_limit_violating_clients": ["client_id"],
  "request_counts_by_client_endpoint_status": [
    {
      "client_id": "client_id",
      "endpoint": "/endpoint",
      "status_code": 200,
      "request_count": 0
    }
  ],
  "rate_limit_counts_by_client": [
    {
      "client_id": "client_id",
      "client_bucket_rate_limit_violation_count": 0,
      "rate_limit_excess_request_count": 0
    }
  ]
}
```

All count fields are nonnegative integers. `total_line_count` counts physical lines, `processed_line_count` counts the nonblank lines examined as JSON, and `valid_request_count` plus `malformed_input_count` equals `processed_line_count`.

The arrays are deterministic: `rate_limit_violating_clients` and `rate_limit_counts_by_client` are sorted by ascending `client_id`; `request_counts_by_client_endpoint_status` is sorted by ascending `(client_id, endpoint, status_code)`. The per-client rate array includes only clients with at least one violating bucket.

## Assumptions and decisions

This is the active decision log for the exercise. It records deliberate interpretations of requirements that the brief leaves open.

- **Input contract:** The program accepts one required positional log-file path. Standard input is not an input mode. File and command-line errors are reported on standard error with a non-zero exit status.
- **Client identity:** `client_id` is globally consistent across all upstream providers. A client rate limit therefore covers that client's traffic across every endpoint and provider.
- **Request ID scope**: `request_id` is not assumed globally unique across upstream providers; it may be unique only within a provider. Because the input has no provider identifier, v1 treats `request_id` as opaque and does not deduplicate records.
- **Why this policy:** A globally consistent client identity makes a client-wide policy meaningful even when requests arrive from different providers. A fixed UTC-aligned ten-second bucket makes the first version simple, deterministic, and directly explainable in a report while still detecting bursts across endpoints.
- **Rate-limit reporting:** The report distinguishes client-time-bucket violations from excess requests. A violating client bucket is counted once when its request count exceeds five; its excess request count is the amount above five. Neither metric claims that an upstream request was blocked.
- **Traffic aggregation:** Request counts are grouped by the exact `client_id`, `endpoint`, and `status_code` combination. The traffic cube contains only those grouping keys and `request_count`; report consumers can roll its rows up to client, endpoint, or status-code views as needed. Client-wide rate-limit metrics remain in a separate client summary because the policy covers all client endpoints together.
- **Malformed input:** A nonblank line is malformed and ignored when it exceeds 4 KiB (4,096 bytes, excluding its LF terminator), is not a JSON object, omits a required field, gives a field the wrong type or an empty required string, contains an invalid RFC 3339 timestamp, or gives a status code outside 100 through 599. The line limit leaves several times the expected space for request IDs, client IDs, and endpoints of up to roughly 256 characters each. Unknown extra fields are accepted within that limit. The endpoint needs no syntax validation beyond being nonempty.
- **Blank input:** Blank or whitespace-only lines are ignored rather than classified as malformed. The report distinguishes the number of physical lines, nonblank processed lines, and ignored blank lines. A single final line terminator does not make an extra blank line; an additional empty line does.
- **Duplicate records:** Version 1 counts every valid log record, including duplicate records. Source-aware idempotency or duplicate detection is a future improvement, because duplicate data may otherwise be indistinguishable from a legitimate retry.
- **Output and failures:** A readable input file always produces exactly one JSON report on standard output, even when it contains malformed records. Command-line or file-access failures write a diagnostic to standard error, exit non-zero, and produce no report.
- **Aggregation resources:** Version 1 streams the file through a reusable current-line buffer capped at 4 KiB plus one byte for overflow detection, and holds aggregate counts and summaries in memory. Oversized lines are drained through their line ending without being retained. Aggregate memory grows with the number of distinct traffic groups and client-time buckets, rather than with the raw input content. Unbounded-cardinality scaling is future work.

## Implementation notes

- JSON parsing and serialization use `serde_json`; RFC 3339 timestamps are parsed with `time` and converted to Unix seconds before fixed-bucket calculation.
- The report uses ordered maps for its aggregate keys, which supplies the documented deterministic array ordering without a second sort pass.
- A record must have nonempty string `request_id`, `timestamp`, `client_id`, and `endpoint` fields, plus an integral status code from 100 through 599. Extra object fields are retained by neither the validation nor the report.
- The implementation deliberately counts valid duplicate lines independently. It reports observed input rather than claiming an upstream system accepted or rejected any particular request.

## Future improvements

- Support client-specific rate-limit policies.
- Add source-aware idempotency or duplicate detection.
- Support bounded-memory aggregation for exceptionally high-cardinality clients and traffic groups, such as an external store or sorted spill files: a file with millions of distinct (client, endpoint, status) groups or (client, UTC bucket) pairs creates millions of map entries, potentially approaching raw-file memory use.
- Externalise configuration such as parameters for rate-limiting.
- Extend the deterministic end-to-end coverage with additional reviewable fixtures for every malformed field and bucket boundary.
- Add `AGENTS.md`, `CLAUDE.md` & `CODING_STANDARDS.md` to store AI & huma guidance for the codebase. 
- Parallel file processing: Split JSONL only on line boundaries, let workers build local aggregates, then merge. For the current fixed-bucket policy, merging (client_id, bucket) counts before evaluating the limit is correct. Sharding by client_id is even better: every client’s rate state lands on one worker.
- Persistence: if reports need to be generated later or over a continuous stream. Store either raw normalized request events, aggregates, or both. Raw events preserve flexibility for new analyses; bucketed aggregates cost less but cannot answer arbitrary new questions later.

## Verification approach

- Cover the public command-line contract with simple end-to-end integration tests.
- Keep named JSON Lines fixtures small and reviewable, including the supplied example and boundary or malformed-input cases.
- Generate a larger deterministic data set within a test to exercise aggregation at greater volume.
- Do not add fuzz testing in this exercise.

## AI assistance disclosure

- Codex/Claude/AI assisted with requirement analysis, paired-TDD coordination, and implementation. The work remains subject to user review.
- Used a custom [agustafson/skills/tdd-pair](https://github.com/agustafson/skills/tree/main/tdd-pair) skill which uses separate agents to write the tests and implementation. 
