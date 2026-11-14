let Contract = import "contract.ncl" in
std.record.update "samples" 101 (import "cohort.ncl") | Contract.Cohort
