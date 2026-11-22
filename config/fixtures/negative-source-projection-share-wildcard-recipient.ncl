# A share names one exact recipient DID, never a wildcard audience.
let contracts = import "../source-projection-contracts.ncl" in
(contracts.share_example & { recipient | force = "did:*" })
  | contracts.Share
