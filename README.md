# API Traffic Report

A command-line program that reads one JSON Lines API request log and writes one JSON traffic report to standard output.

## Assumptions and decisions

This is the active decision log for the exercise. It records deliberate interpretations of requirements that the brief leaves open.

- **Input contract:** The program accepts one required positional log-file path. Standard input is not an input mode. File and command-line errors are reported on standard error with a non-zero exit status.
- **Client identity:** `client_id` is globally consistent across all upstream providers. A client rate limit therefore covers that client's traffic across every endpoint and provider.
- **Rate-limit scope:** One globally configured policy applies to every client. Version 1 does not need command-line configuration or a configuration file. The limit is five requests per fixed, UTC-aligned ten-second bucket, with a bucket start included and its end excluded. A client is evaluated across every endpoint and provider.
- **Rate-limit reporting:** The report distinguishes client-time-bucket violations from excess requests. A violating client bucket is counted once when its request count exceeds five; its excess request count is the amount above five. Neither metric claims that an upstream request was blocked.
- **Traffic aggregation:** Request counts are grouped by the exact `client_id`, `endpoint`, and `status_code` combination. The traffic cube contains only those grouping keys and `request_count`; report consumers can roll its rows up to client, endpoint, or status-code views as needed. Client-wide rate-limit metrics remain in a separate client summary because the policy covers all client endpoints together.
- **Malformed input:** A nonblank line is malformed and ignored when it is not a JSON object, omits a required field, gives a field the wrong type or an empty required string, contains an invalid RFC 3339 timestamp, or gives a status code outside 100 through 599. Unknown extra fields are accepted. The endpoint needs no syntax validation beyond being nonempty.
- **Blank input:** Blank or whitespace-only lines are ignored rather than classified as malformed. The report distinguishes the number of physical lines, nonblank processed lines, and ignored blank lines. A single final line terminator does not make an extra blank line; an additional empty line does.
- **Duplicate records:** Version 1 counts every valid log record, including duplicate records. Source-aware idempotency or duplicate detection is a future improvement, because duplicate data may otherwise be indistinguishable from a legitimate retry.
- **Output and failures:** A readable input file always produces exactly one JSON report on standard output, even when it contains malformed records. Command-line or file-access failures write a diagnostic to standard error, exit non-zero, and produce no report.
- **Aggregation resources:** Version 1 holds aggregate counts and summaries in memory. Its memory usage grows with the number of distinct traffic groups and client-time buckets, rather than with the raw input content; unbounded-cardinality scaling is future work.

## Future improvements

- Support client-specific rate-limit policies.
- Add source-aware idempotency or duplicate detection.

## Verification approach

- Cover the public command-line contract with simple end-to-end integration tests.
- Keep named JSON Lines fixtures small and reviewable, including the supplied example and boundary or malformed-input cases.
- Generate a larger deterministic data set within a test to exercise aggregation at greater volume.
- Do not add fuzz testing in this exercise.
