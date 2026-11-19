let H = import "../helpers.ncl" in
H.mapping "ucan-wg-1.0.0" [{ command = "/unknown", action = "document-read", subject_class = "exact", resource_binding = "external-owner" }] H.non_claims
