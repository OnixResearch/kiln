let NonEmptyString = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value > 0
) in
let Applicability = std.contract.from_predicate (fun value =>
  std.is_string value && std.array.elem value ["always", "when-supported"]
) in
let NegativeCase = {
  id | NonEmptyString,
  mutation | NonEmptyString,
  expected_code | NonEmptyString,
  applicability | Applicability,
} in
[
  { id = "wrong-provider-identity", mutation = "change the Octet revision or provider closure identity", expected_code = "provider-identity", applicability = "always" },
  { id = "wrong-extraction-identity", mutation = "change the effective extraction contract", expected_code = "extraction-identity", applicability = "always" },
  { id = "wrong-profile-identity", mutation = "change the Octet profile BLAKE3", expected_code = "profile-identity", applicability = "always" },
  { id = "source-mismatch", mutation = "change production source bytes", expected_code = "source-identity", applicability = "always" },
  { id = "shadow-source", mutation = "select a copied source path", expected_code = "source-path", applicability = "always" },
  { id = "has-errors", mutation = "set Charon has_errors", expected_code = "extraction-errors", applicability = "always" },
  { id = "skip-borrowck", mutation = "set Charon skip_borrowck", expected_code = "borrow-check", applicability = "always" },
  { id = "target-drift", mutation = "change target triple, width, or endianness", expected_code = "target-metadata", applicability = "always" },
  { id = "stale-manifest", mutation = "retain an old manifest identity", expected_code = "manifest-freshness", applicability = "always" },
  { id = "cross-run-def-id", mutation = "promote a run-local def_id to durable identity", expected_code = "run-local-id", applicability = "always" },
  { id = "tool-failure", mutation = "replace a source support outcome with process failure", expected_code = "outcome-class", applicability = "always" },
  { id = "budget-exhaustion", mutation = "exceed a named stage or artifact budget", expected_code = "budget-outcome", applicability = "always" },
  { id = "unsupported-const-fn", mutation = "classify const fn as unsupported without an exact diagnostic", expected_code = "unsupported-boundary", applicability = "always" },
  { id = "unsupported-result", mutation = "classify Result as unsupported without an exact diagnostic", expected_code = "unsupported-boundary", applicability = "always" },
  { id = "unsupported-error-representation", mutation = "classify the error enum as unsupported without an exact diagnostic", expected_code = "unsupported-boundary", applicability = "always" },
  { id = "stale-llbc", mutation = "change LLBC after extraction", expected_code = "llbc-identity", applicability = "when-supported" },
  { id = "stale-translation-manifest", mutation = "change translation.json rows", expected_code = "translation-manifest", applicability = "when-supported" },
  { id = "stale-generated-lean", mutation = "change generated Lean bytes", expected_code = "generated-boundary", applicability = "when-supported" },
  { id = "wrong-theorem-identity", mutation = "change theorem or statement approval", expected_code = "theorem-identity", applicability = "when-supported" },
  { id = "wrong-requirement-or-claim", mutation = "change requirement or claim class", expected_code = "claim-identity", applicability = "when-supported" },
  { id = "undeclared-trust-input", mutation = "add or change a model, axiom, or opaque definition", expected_code = "trust-closure", applicability = "when-supported" },
  { id = "swapped-receipt", mutation = "substitute another Octet receipt", expected_code = "receipt-identity", applicability = "when-supported" },
  { id = "absent-artifact-verification", mutation = "omit Octet artifact verification", expected_code = "artifact-verification", applicability = "when-supported" },
  { id = "false-theorem", mutation = "select an intentionally false theorem", expected_code = "lean-check", applicability = "when-supported" },
  { id = "vacuous-theorem", mutation = "select a True-style theorem", expected_code = "theorem-adequacy", applicability = "when-supported" },
  { id = "overclaim-text", mutation = "claim Aeneas proof success for a blocked receipt", expected_code = "claim-boundary", applicability = "always" },
] | Array NegativeCase
