# Must fail: the spindle is a did:web service DID, the token audience.
(import "tangled-spindle-positive.ncl")
& {
  spindle_did | force = "did:plc:abcdefghijklmnopqrstuvwx",
}
