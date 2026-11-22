# Publication is a protected implementation operation, not a source ability.
let contracts = import "../source-projection-contracts.ncl" in
(contracts.share_example & { operations | force = ['discover, 'publish] })
  | contracts.Share
