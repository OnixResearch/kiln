let Data = import "quartet-data.ncl" in
std.record.update
  "roles"
  (
    std.record.update
      "candidate"
      (std.record.update "changes" ["change-base", "change-a", "change-a"] Data.roles.candidate)
      Data.roles
  )
  Data
  | (import "contract.ncl").Fixture
