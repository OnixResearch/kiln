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
let AgentText =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.trim value == value
  )
in
let AtMost = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_array value && std.array.length value <= maximum
  )
in
let NonEmptyAtMost = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_array value && std.array.length value >= 1 && std.array.length value <= maximum
  )
in
let UniqueViewerIds =
  std.contract.from_predicate (fun value =>
    let check = fun state viewer =>
      if std.array.elem viewer.id state.seen then
        { seen = state.seen, duplicate = true }
      else
        { seen = state.seen @ [viewer.id], duplicate = false }
    in
    !(std.array.fold_left check { seen = [], duplicate = false } value.viewers).duplicate
  )
in
let UniquePrincipals =
  std.contract.from_predicate (fun value =>
    if value.proxy == null then
      true
    else
      let check = fun state principal =>
        if std.array.elem principal.user_id state.users
        || std.array.elem principal.login_name state.logins then
          { users = state.users, logins = state.logins, duplicate = true }
        else
          {
            users = state.users @ [principal.user_id],
            logins = state.logins @ [principal.login_name],
            duplicate = false,
          }
      in
      !(std.array.fold_left check { users = [], logins = [], duplicate = false } value.proxy.principals).duplicate
  )
in
fun input =>
  let settings =
    input
      | {
        viewers
          | Array {
            id | Identifier,
            label | Label,
            token_file | c.AbsolutePath,
            agent
              | {
                harness | Label | AgentText,
                model | Label | AgentText | optional,
              }
              | optional,
          }
          | AtMost 128
          | default
          = [],
        proxy | Dyn | default = null,
      }
      | UniqueViewerIds
      | UniquePrincipals
  in
  let admitted_proxy =
    if settings.proxy == null then
      null
    else
      settings.proxy
        | {
          listen_address
            | std.contract.from_predicate (fun value =>
              std.is_string value
              && std.string.is_match "^(127\\.0\\.0\\.1|\\[::1\\]):[1-9][0-9]*$" value
            ),
          credential_file | c.AbsolutePath,
          tailscaled_socket_path | c.AbsolutePath,
          principals
            | Array {
              user_id | UserID,
              login_name | Label,
              label | Label,
              role | Role,
              agent | { harness | Label | AgentText, model | Label | AgentText | optional } | optional,
            }
            | NonEmptyAtMost 128,
        }
  in
  {
    schema = "kiln.web-access.v1",
    viewers = settings.viewers,
    proxy = admitted_proxy,
  }
