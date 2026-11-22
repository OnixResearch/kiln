# A source identity that is not a public token.
(import "../inventory-profile.ncl") {
  principal = "operator",
  sources = [
    {
      source = "../animus",
      producer = "animus",
      scope = "tenant-pilot",
      kind = "serving",
      minimum_generation = 1,
    },
  ],
}
