let H = import "../helpers.ncl" in
H.mapping "ucan-wg-1.0.0" (H.entries @ [{ command = "/document/read", action = "document-write", subject_class = "exact", resource_binding = "external-owner" }]) H.non_claims
