# Path scopes are literal components; `..` cannot climb out of a grant.
let contracts = import "../source-projection-contracts.ncl" in
(contracts.share_example & { paths | force = [{ components = ["crates", ".."], subtree = true }] })
  | contracts.Share
