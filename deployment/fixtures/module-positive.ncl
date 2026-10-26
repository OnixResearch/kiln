# Caller supplies the reviewed Onix library on Nickel's import path.
let onix = import "onix.ncl" in
let service = (import "../onixos-module.ncl") onix in
let inputs = import "inputs.ncl" in
service.impl (
  onix.testing.mock_args {
    service = "kiln-web-service",
    role_name = 'server,
    settings = inputs & { uid = 12345, gid = 12345 },
    packages = { kiln-web-service = "fixture-package-handle" },
  }
)
