# Product-owned external Onix module. No target is selected or activated here.
fun onix =>
  let contracts = import "contracts.ncl" in
  let build_systemd = import "systemd.ncl" in
  ({
    interface = {
      manifest = {
        name = "kiln/web-service",
        description = "Single-tenant Kiln HTTPS service with systemd credentials and readiness.",
        categories = ["ci", "web"],
      },
      portability = {
        status = 'platform_specific,
        requires = { typed_namespaces = ["state"] },
        platform_leaves = [
          {
            namespace = "nixos",
            targets = ["nixos", "mobile_nixos", "apple_silicon"],
            rationale = "Requires systemd Type=notify, native credential mounts and Linux hardening; no portable notify provider exists.",
          }
        ],
      },
      packages = ["kiln-web-service"],
      constraints = { maxInstances = 1, roles.server = { minMachines = 1, maxMachines = 1 } },
      roles.server =
        contracts.Settings
        & {
          uid | contracts.AccountId | doc "Operator-reserved dedicated non-root numeric UID; must match repository/proxy ACLs.",
          gid | contracts.AccountId | doc "Operator-reserved dedicated numeric GID; no supplementary groups are granted.",
        },
    },
    impl = fun args =>
      let cfg = args.settings | interface.roles.server in
      {
        output = {
          nixos = {
            users.groups.kiln-web-service.gid = cfg.gid,
            users.users.kiln-web-service = {
              isSystemUser = true,
              uid = cfg.uid,
              group = "kiln-web-service",
              home = "/var/lib/kiln-web-service",
              createHome = false,
              description = "Kiln HTTPS service",
            },
            systemd.services.kiln-web-service =
              (
                build_systemd {
                  profile_path = cfg.profile_path,
                  credential_sources = cfg.credential_sources,
                }
              )
              & { path = [args.packages.kiln-web-service] },
          },
          state.directories.web-service = {
            path = "/var/lib/kiln-web-service",
            owner = "kiln-web-service",
            group = "kiln-web-service",
            mode = "0700",
            durability = 'preserved,
            scope = 'machine,
          },
        },
      },
  }
  ) | onix.ServiceModule
