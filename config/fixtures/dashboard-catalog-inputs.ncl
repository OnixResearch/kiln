{
  schema_version = 1,
  bounds = {
    max_catalog_bytes = 1048576,
    max_projects = 32,
    max_histories = 128,
    max_input_bytes = 268435456,
    max_rendered_bytes = 67108864,
  },
  projects = [
    {
      id = "kiln",
      label = "Operator-declared Kiln",
      histories = [
        { id = "retained", label = "Retained journal", source = "Operator-declared source", revision = "Operator-declared revision", input = { kind = "journal", path = "journals/retained.json" } },
        { id = "store", label = "Canonical local stream", input = { kind = "store", path = "events.sqlite", stream = "kiln:ci:retained" } },
      ],
    }
  ],
}
