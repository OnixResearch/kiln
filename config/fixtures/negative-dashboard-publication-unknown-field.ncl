# r[verify kiln.dashboard.repositories.delivery.scenario.failure]
let Publication = import "../dashboard-publication.ncl" in
let Valid = import "dashboard-publication-positive.ncl" in
Publication (std.record.insert "unexpected_field" true Valid)
