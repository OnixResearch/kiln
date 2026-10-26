# r[verify kiln.dashboard.repositories.delivery.scenario.failure]
let Publication = import "../dashboard-publication.ncl" in
let Valid = import "dashboard-publication-positive.ncl" in
Publication (std.record.insert "subprocess_timeout_seconds" 0 (std.record.remove "subprocess_timeout_seconds" Valid))
