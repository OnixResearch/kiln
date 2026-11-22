# A glob is a filter, not a resource: it cannot widen a shared path.
let contracts = import "../source-projection-contracts.ncl" in
(contracts.share_example & { paths | force = [{ components = ["crates", "*"], subtree = true }] })
  | contracts.Share
