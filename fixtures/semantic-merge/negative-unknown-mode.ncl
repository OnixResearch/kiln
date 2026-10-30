let Data = import "quartet-data.ncl" in
std.record.update "policy" (std.record.update "mode" "optional" Data.policy) Data
  | (import "contract.ncl").Fixture
