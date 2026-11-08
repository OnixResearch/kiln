let positive = import "web-service-positive.ncl" in
(import "../web-service.ncl") (positive & { feeds | force = { token_file = positive.operator_token_file, max_subscriptions = 1, max_entries_per_feed = 1, max_feed_bytes = 4096, max_redirects = 0, timeout_seconds = 1 } })
