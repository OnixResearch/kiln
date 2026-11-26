## Context

Herdr window browsing spans an authenticated operator console and a native authority.

## Decisions

Use only configured Herdr hosts and its native window and capture operations. Only an authenticated operator can read terminal contents. Use the selected host and a window observed in that host’s current inventory, bounded sanitized text, and explicit unavailable states.

## Risks / Trade-offs

Absent capabilities fail closed. Reusing provider admission avoids a second source of truth.
