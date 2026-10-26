# r[verify kiln.dashboard.live.collection.scenario.admission]
let Refresh = import "../dashboard-refresh.ncl" in
let Valid = import "dashboard-refresh-positive.ncl" in
Refresh (Valid & { unexpected = true })
