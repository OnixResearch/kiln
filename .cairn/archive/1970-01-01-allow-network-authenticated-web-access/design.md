## Context

The installed service authenticates bearer credentials and binds its existing profile to durable run state. The tailnet proxy already validates the public Host and Origin and validates backend TLS. Replacing the service origin, dropping backend authentication or exposing the bearer in HTML would either break durable linkage or widen credential exposure.

## Decisions

### Decision: Discover an authorized network session through the existing API

The browser performs one bounded, same-origin GET /api/projects without Authorization. A validated success admits a network session and loads the normal workspace. A 401 leaves manual token login unchanged. Discovery is read-only, not retried and covered by the existing request timeout, response bound and session epoch fences. Ordinary API calls require an admitted session and attach Authorization only for a manually supplied bearer session.

### Decision: Keep authority in the tailnet shell

Only the HTTPS listener bound to the declared Tailscale addresses injects the existing operator credential for backend requests. A private startup-generated Nginx include lives in the user runtime directory, is never put in the Nix store, browser content, URLs, logs or source control, and is retired with the proxy unit. Cross-origin and missing-origin writes remain rejected. Loopback service credentials and worker role restrictions are unchanged; canonical profiles and run state are not migrated.

### Decision: Do not pretend network access can sign out of the tailnet

The network session identifies its access mode and does not offer bearer sign-out. Leaving the page clears all browser state and pending requests; a restored page must discover current network authorization afresh. Manual bearer sessions retain explicit sign-out and do not regain their cleared bearer on restoration.

## Risks / Trade-offs

Any peer allowed to reach the tailnet listener can start and control CI with the operator account's authority. The operator owns tailnet ACLs, certificate renewal and credential rotation. A network session is not per-user identity. The backend token remains a secret held by the proxy, rather than a value disclosed to the browser. Source and deployment evidence remain separately scoped.
