# The root is only a subtree; an exact scope must name a path.
let contracts = import "../source-projection-contracts.ncl" in
(contracts.share_example & { paths | force = [{ components = [], subtree = false }] })
  | contracts.Share
