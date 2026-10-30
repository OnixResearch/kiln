let Data = import "quartet-data.ncl" in
std.record.update
  "subject"
  (std.record.update "required_coverage" ["crate::sub", "crate::add"] Data.subject)
  Data
  | (import "contract.ncl").Fixture
