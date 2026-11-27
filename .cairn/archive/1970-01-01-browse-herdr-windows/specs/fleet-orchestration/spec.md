## ADDED Requirements

### Requirement: Herdr window browsing

r[kiln.sdlc.herdr_windows]
Kiln MUST inspect bounded terminal windows on configured Herdr hosts, including windows without a Kiln attempt. Use only configured Herdr hosts and its native window and capture operations. Only an authenticated operator can read terminal contents. Use the selected host and a window observed in that host’s current inventory, bounded sanitized text, and explicit unavailable states.

#### Scenario: Operator uses herdr window browsing

r[kiln.sdlc.herdr_windows.scenario.operator]
- GIVEN an authenticated operator and an explicitly configured provider
- WHEN they open a configured host and select an observed terminal window
- THEN Kiln MUST show the exact host and window with bounded captured output; refuse unknown windows and viewer capture
