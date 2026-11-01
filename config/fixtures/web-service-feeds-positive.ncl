(import "../web-service.ncl") (
  (import "web-service-inputs.ncl")
  & {
    feeds = {
      token_file = "/etc/kiln-web-service/credentials/feed.token",
      max_subscriptions = 32,
      max_entries_per_feed = 50,
      max_feed_bytes = 1048576,
      max_redirects = 3,
      timeout_seconds = 10,
      refresh_interval_ms = 900000,
      allow_private_destinations = false,
    },
  }
)
