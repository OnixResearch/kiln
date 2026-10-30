# Must fail: clone URLs are HTTPS only.
(import "tangled-spindle-positive.ncl")
& {
  repositories | force = [
    {
      repo_did = "did:plc:abcdefghijklmnopqrstuvwx",
      knot_did = "did:web:knot.example.com",
      clone_url = "http://knot.example.com/did:plc:abcdefghijklmnopqrstuvwx",
      trusted_actors = [],
      approvers = ["did:plc:approver234567abcdefghij"],
    },
  ],
}
