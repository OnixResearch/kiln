# A child that may delegate must keep depth to delegate with.
let contracts = import "../source-projection-contracts.ncl" in
(contracts.share_example & { operations | force = ['discover, 'delegate] })
  | contracts.Share
