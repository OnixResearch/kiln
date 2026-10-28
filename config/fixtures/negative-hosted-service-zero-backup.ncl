let positive = import "hosted-service-positive.ncl" in
(import "../hosted-service.ncl") (
  positive
  & {
    operations | force = positive.operations & { maximum_backup_ms | force = 0 },
  }
)
