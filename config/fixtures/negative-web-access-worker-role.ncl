let input = import "web-access-inputs.ncl" in
(import "../web-access.ncl") (
  input
  & {
    proxy | force =
      input.proxy
      & {
        principals | force = [
          {
            user_id = "47256370424925063",
            login_name = "operator@example.com",
            label = "Unsupported worker identity",
            role = "worker",
          }
        ],
      },
  }
)
