let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "malformed-stale-catalog",
    support_catalog_blake3 = "12fb9fec563b5df9f40a58bbbddfd0bcf5abe74da373ae0b506712647e0ee6ad",
  }
) | Contracts.Fixture
