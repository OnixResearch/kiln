## Why

Inspect bounded terminal windows on configured Herdr hosts, including windows without a Kiln attempt. Today the console lacks this control.

## What Changes

Use only configured Herdr hosts and its native window and capture operations. Only an authenticated operator can read terminal contents. Use the selected host and a window observed in that host’s current inventory, bounded sanitized text, and explicit unavailable states.

## Impact

Kiln owns the operator surface and receipts; the named provider retains authority over effects.
