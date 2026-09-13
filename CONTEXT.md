# API Traffic Reporting

This context turns provider-produced API request logs into a report of observed traffic, input quality, and client rate-limit violations.

## Language

**Request record**:
A valid JSON object from a nonblank log line that describes one observed API request.
_Avoid_: Event, entry

**Client**:
The globally consistent identifier for the party that made an API request, regardless of the upstream provider that supplied the log.
_Avoid_: Account, customer, caller

**Rate-limit policy**:
A globally configured rule that limits a client's aggregate API request frequency across all endpoints in fixed UTC-aligned time buckets.
_Avoid_: Strategy, quota

**Time bucket**:
A fixed UTC-aligned interval in which request frequency is evaluated for rate limiting.
_Avoid_: Rolling window, request window

**Rate-limit violation**:
A single client-time-bucket occurrence in which the client's request frequency exceeds the applicable rate-limit policy.
_Avoid_: Breach, flagged request

**Rate-limit excess request**:
A request above the policy's permitted count within a violating client-time bucket; it is a calculated overage, not evidence that the request was rejected.
_Avoid_: Rate-limited request, blocked request

**Malformed input**:
A nonblank log line that cannot produce a request record because its required JSON structure or field values are invalid.
_Avoid_: Bad request, invalid request
