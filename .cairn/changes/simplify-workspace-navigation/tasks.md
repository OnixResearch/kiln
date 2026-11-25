## Phase 1: Implementation

- [x] [serial] Simplify the rail while retaining every destination. r[kiln.web.primary_navigation]
- [x] [serial] Qualify route and responsive behavior in the browser. r[kiln.web.primary_navigation]

Verification: Built `kiln-web-service --example explore` and exercised the
loopback fixture in Chromium. The default rail showed five direct links and
one disclosure. Workers opened from the disclosure, Runs closed it, browser
Back restored it, a direct Operations URL revealed its current link, and
numbered command search opened Source. At 390 px the mobile drawer skipped
closed tools on Tab, closed when navigating to My inbox, and returned focus
to the toggle on Escape without horizontal overflow.
