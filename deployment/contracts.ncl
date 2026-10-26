# Deployment contracts; no credential content is accepted by this interface.
let is_path = fun value =>
  std.is_string value
  && std.string.is_match "^/([A-Za-z0-9_+-][A-Za-z0-9_.+-]*/)*[A-Za-z0-9_+-][A-Za-z0-9_.+-]*$" value
in
let Path = std.contract.from_predicate is_path in
let RuntimeSource =
  std.contract.from_predicate (fun value =>
    is_path value
    && !(std.string.is_match "^/nix/store(/|$)" value)
    && !(std.string.is_match "^/var/lib/kiln-web-service(/|$)" value)
  )
in
{
  AbsoluteCommandPath = Path,
  CredentialSource = RuntimeSource,
  AccountId =
    std.contract.from_predicate (fun value =>
      std.is_number value
      && std.number.floor value == value
      && value >= 1
      && value <= 4294967294
    ),
  Settings = {
    profile_path | Path | doc "Reviewed secret-free runtime JSON profile; no automatic profile generation or migration.",
    credential_sources
      | {
        tls-certificate | RuntimeSource,
        tls-key | RuntimeSource,
        operator-token | RuntimeSource,
        worker-token | RuntimeSource,
      },
  },
}
