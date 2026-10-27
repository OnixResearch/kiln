# Optional separate runtime access policy; export JSON and pass --access explicitly.
# Credentials are private owned files containing exactly 64 ASCII hex bytes, no newline.
# nginx auth_request calls the private loopback proxy-auth helper, never the browser.
let c = import "github-contracts.ncl" in
let Label =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 128
    && std.string.is_match "^[ -~]+$" value
  )
in
let Identifier =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 128
    && std.string.is_match "^[A-Za-z0-9_.-]+$" value
  )
in
let UserID =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 20
    && std.string.is_match "^[1-9][0-9]*$" value
  )
in
let Role = std.contract.from_predicate (fun value => value == "viewer" || value == "operator") in
fun input =>
  let settings =
    input
      | {
        viewers | Array { id | Identifier, label | Label, token_file | c.AbsolutePath } | default = [],
        proxy | Dyn | default = null,
      }
  in
  let admitted_proxy =
    if settings.proxy == null then
      null
    else
      settings.proxy
        | {
          listen_address
            | std.contract.from_predicate (fun value =>
              std.is_string value && std.string.is_match "^(127\\.0\\.0\\.1|\\[::1\\]):[0-9]+$" value
            ),
          credential_file | c.AbsolutePath,
          tailscaled_socket_path | c.AbsolutePath,
          principals
            | Array {
              user_id | UserID,
              login_name | Label,
              label | Label,
              role | Role,
            },
        }
  in
  {
    schema = "kiln.web-access.v1",
    viewers = settings.viewers,
    proxy = admitted_proxy,
  }
