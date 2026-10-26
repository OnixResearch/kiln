# Bind deployment-only paths, then delegate all application validation/vocabulary.
# Input must omit tls, operator_token_file, worker_token_file and state_directory.
# Existing durable state is bound to this exact exported profile; never silently migrate it.
let build = import "web-service.ncl" in
fun application =>
  build (
    application
    & {
      tls = {
        certificate_path = "/run/credentials/kiln-web-service.service/tls-certificate",
        private_key_path = "/run/credentials/kiln-web-service.service/tls-key",
        min_version = "TLS1.3",
      },
      operator_token_file = "/run/credentials/kiln-web-service.service/operator-token",
      worker_token_file = "/run/credentials/kiln-web-service.service/worker-token",
      state_directory = "/var/lib/kiln-web-service",
    }
  )
