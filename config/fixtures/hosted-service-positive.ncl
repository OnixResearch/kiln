# Authoring-contract fixture only: these tool/image/key paths are not provisioned.
# Exporting this fixture is not executable/backend or hostile-isolation evidence.
let identity = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef" in
let executable = { path = "/nix/store/fixture-not-a-runtime/bin/tool", blake3 = identity } in
(import "../hosted-service.ncl") {
  service_id = "fixture",
  state_directory = "/private/state",
  network = {
    key_file = "/private/server.key",
    bind_address = "127.0.0.1:44771",
    relay_urls = [],
  },
  bootstrap_peer = identity,
  bootstrap_organization = "acme",
  operator_peers = [identity],
  source_profiles = [
    {
      path = "/private/source.json",
      blake3 = identity,
      organization = "acme",
      project = "app",
      network_activation = false,
      price_id = identity,
      class = "local",
      max_units = 1000,
    }
  ],
  operations = {
    service_id = "fixture",
    backup_root = "/private/backup",
    restore_root = "/private/restore",
    content_root = "/private/content",
    replay_roots = ["/private/replay"],
    maximum_database_bytes = 1048576,
    maximum_content_bytes = 1024,
    maximum_content_records = 10,
    maximum_backup_ms = 1000,
  },
  secrets = {
    issuer = "https://issuer.example",
    signing_key_path = "/private/sign.key",
    audiences = ["app"],
    token_lifetime_seconds = 60,
    resolvers = [
      {
        id = "app-resolver",
        organization = "acme",
        project = "app",
        reference = {
          program_path = "/nix/store/fixture-not-a-runtime/bin/secretspec",
          program_blake3 = identity,
          project_root = "/private/secrets",
          profile = "fixture",
          provider = null,
          secret_key = "fixture-key",
        },
      }
    ],
  },
  execution = {
    systemd_run = executable,
    systemctl = executable,
    manager_runtime_directory = "/run/user/1000",
    env = executable,
    work_root = "/private/work",
    cache = {
      directory = "/private/cache",
      maximum_entry_bytes = 4096,
      maximum_total_bytes = 65536,
      maximum_entries = 16,
      maximum_age_seconds = 3600,
      maximum_files = 16,
      policies = [
        {
          organization = "acme",
          project = "app",
          runner = "local",
          worker = {
            producer = "hosted",
            toolchain = "compiler-v1",
            platform = "linux",
            inputs = ["Cargo.lock"],
            paths = ["build-cache"],
            maximum_entry_bytes = 4096,
            maximum_files = 16,
          },
        }
      ],
    },
    trusted_projects = [{ organization = "acme", project = "app" }],
    pricing = [
      {
        organization = "acme",
        runner = "local",
        price_id = identity,
        class = "local",
        max_units = 1000,
      }
    ],
    workload = [
      {
        organization = "acme",
        project = "app",
        runner = "local",
        audience = "app",
        secrets = [{ environment = "APP_TOKEN", name = "app-token", revision = 1 }],
      }
    ],
    runners = {
      local = {
        kind = "namespaces",
        unshare = executable,
        python = executable,
        mount = executable,
        shell = executable,
        runtime_roots = ["/nix/store"],
      },
      vm = {
        kind = "qemu",
        qemu = executable,
        kernel = executable,
        initramfs = executable,
        root_image = executable,
        guest_shell = "/bin/sh",
        acceleration = "tcg",
        qualification_identity = null,
      },
      microvm = {
        kind = "firecracker",
        firecracker = executable,
        kernel = executable,
        initrd = executable,
        guest_shell = "/bin/bash",
        vcpus = 1,
        memory_mib = 256,
        boot_timeout_ms = 10000,
        source_image_mib = 64,
        warm_pool = { directory = "/private/microvm-pool", slots = 1 },
        qualification_identity = null,
      },
    },
    maximum_concurrent = 1,
    memory_bytes = 67108864,
    processes = 8,
    cpu_percent = 100,
    maximum_timeout_ms = 1000,
    maximum_output_bytes = 1024,
  },
  fleet = {
    driver = executable,
    organizations = ["acme"],
    pricing = { price_id = identity, class = "fleet", max_units = 100 },
    minimum = 0,
    maximum = 1,
    slots_per_instance = 1,
    timeout_ms = 1000,
  },
}
