let H = import "../helpers.ncl" in
std.record.merge H.mappings {
  molten.mappings = std.array.take 1 H.mappings.molten.mappings,
} | (import "../../contracts.ncl").ConsumerMappings
