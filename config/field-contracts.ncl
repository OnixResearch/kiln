# Shared exact-field admission for hosted Nickel records.
{
  ExactFields = fun allowed =>
    fun value =>
      std.array.all (fun field => std.array.elem field allowed) (std.record.fields value),
}
