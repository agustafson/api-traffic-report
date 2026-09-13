//! Accumulates valid request records and reduces them into the output report.

use std::collections::BTreeMap;

use serde::Serialize;

use super::ids::{ClientId, Endpoint, StatusCode};
use super::{REQUESTS_PER_BUCKET, record::RequestRecord};

/// The complete JSON traffic report, in the field order the CLI contract fixes.
#[derive(Serialize)]
pub(crate) struct Report {
    total_line_count: u64,
    processed_line_count: u64,
    valid_request_count: u64,
    malformed_input_count: u64,
    ignored_blank_line_count: u64,
    client_bucket_rate_limit_violation_count: u64,
    rate_limit_excess_request_count: u64,
    rate_limit_violating_clients: Vec<ClientId>,
    request_counts_by_client_endpoint_status: Vec<RequestCount>,
    rate_limit_counts_by_client: Vec<ClientRateLimitCount>,
}

/// One row of the traffic cube: a request tally for one `(client, endpoint, status)` group.
#[derive(Serialize)]
struct RequestCount {
    client_id: ClientId,
    endpoint: Endpoint,
    status_code: StatusCode,
    request_count: u64,
}

/// One client's rate-limit summary across all of its buckets.
#[derive(Serialize)]
struct ClientRateLimitCount {
    client_id: ClientId,
    client_bucket_rate_limit_violation_count: u64,
    rate_limit_excess_request_count: u64,
}

/// Traffic-cube grouping key; `Ord` gives `request_counts` its documented sort order for free.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct RequestGroupKey {
    client_id: ClientId,
    endpoint: Endpoint,
    status_code: StatusCode,
}

/// A client's ten-second UTC bucket, keying how many requests it saw in that window.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct ClientBucketKey {
    client_id: ClientId,
    bucket: i64,
}

/// Streaming accumulator that folds each parsed line into the running aggregates.
#[derive(Default)]
pub(super) struct ReportAccumulator {
    total_line_count: u64,
    valid_request_count: u64,
    malformed_input_count: u64,
    ignored_blank_line_count: u64,
    request_counts: BTreeMap<RequestGroupKey, u64>,
    bucket_counts: BTreeMap<ClientBucketKey, u64>,
}

impl ReportAccumulator {
    pub(super) fn received_line(&mut self) {
        self.total_line_count += 1;
    }

    pub(super) fn received_blank_line(&mut self) {
        self.ignored_blank_line_count += 1;
    }

    pub(super) fn received_malformed_input(&mut self) {
        self.malformed_input_count += 1;
    }

    pub(super) fn add_request(&mut self, record: RequestRecord) {
        self.valid_request_count += 1;
        *self
            .request_counts
            .entry(RequestGroupKey {
                client_id: record.client_id.clone(),
                endpoint: record.endpoint.clone(),
                status_code: record.status_code,
            })
            .or_insert(0) += 1;
        *self
            .bucket_counts
            .entry(ClientBucketKey {
                client_id: record.client_id,
                bucket: record.bucket,
            })
            .or_insert(0) += 1;
    }

    pub(super) fn finish(self) -> Report {
        let request_counts_by_client_endpoint_status = self
            .request_counts
            .into_iter()
            .map(|(key, request_count)| RequestCount {
                client_id: key.client_id,
                endpoint: key.endpoint,
                status_code: key.status_code,
                request_count,
            })
            .collect();

        let mut client_rate_counts: BTreeMap<ClientId, (u64, u64)> = BTreeMap::new();
        for (key, count) in self.bucket_counts {
            if count > REQUESTS_PER_BUCKET {
                let entry = client_rate_counts.entry(key.client_id).or_insert((0, 0));
                entry.0 += 1;
                entry.1 += count - REQUESTS_PER_BUCKET;
            }
        }

        Report {
            total_line_count: self.total_line_count,
            processed_line_count: self.valid_request_count + self.malformed_input_count,
            valid_request_count: self.valid_request_count,
            malformed_input_count: self.malformed_input_count,
            ignored_blank_line_count: self.ignored_blank_line_count,
            client_bucket_rate_limit_violation_count: client_rate_counts
                .values()
                .map(|(violations, _)| violations)
                .sum(),
            rate_limit_excess_request_count: client_rate_counts
                .values()
                .map(|(_, excess)| excess)
                .sum(),
            rate_limit_violating_clients: client_rate_counts.keys().cloned().collect(),
            request_counts_by_client_endpoint_status,
            rate_limit_counts_by_client: client_rate_counts
                .into_iter()
                .map(
                    |(
                        client_id,
                        (client_bucket_rate_limit_violation_count, rate_limit_excess_request_count),
                    )| ClientRateLimitCount {
                        client_id,
                        client_bucket_rate_limit_violation_count,
                        rate_limit_excess_request_count,
                    },
                )
                .collect(),
        }
    }
}
