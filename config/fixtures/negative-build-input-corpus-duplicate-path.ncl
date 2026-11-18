# A corpus that lists one path twice is refused.
let contracts = import "../build-input-contracts.ncl" in
(
  contracts.example
  & {
    files | force = contracts.example.files @ [std.array.first contracts.example.files],
  }
)
  | contracts.Corpus
  | contracts.Consistent
