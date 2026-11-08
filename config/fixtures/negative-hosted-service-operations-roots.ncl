let positive = import "hosted-service-positive.ncl" in
(import "../hosted-service.ncl") (positive & { operations | force = positive.operations & { restore_root | force = positive.operations.backup_root } })
