let Data = import "quartet-data.ncl" in
std.record.update
  "policy"
  (
    std.record.update
      "retry"
      (std.record.update "max_attempts" 0 Data.policy.retry)
      Data.policy
  )
  Data
  | (import "contract.ncl").Fixture
