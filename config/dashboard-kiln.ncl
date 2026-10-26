# Kiln's actual retained self-CI observations, not the synthetic preview.
# Regenerate JSON alongside this file so relative paths remain deterministic.
# Exported GitHub journals are acquired from the original exact-profile replay DB.
(import "dashboard-catalog.ncl") {
  schema_version = 1,
  bounds = {
    max_catalog_bytes = 1048576,
    max_projects = 4,
    max_histories = 16,
    max_input_bytes = 16777216,
    max_rendered_bytes = 16777216,
  },
  projects = [
    {
      id = "kiln-github",
      label = "Kiln / GitHub CI history",
      histories = [
        {
          id = "pass",
          label = "Retained real build: passed",
          source = "Retained GitHub run for onixresearch/kiln; not a current branch-tip claim",
          revision = "e25b7c1a1faac1e9f4eaea24bd9c7cd916ebb817",
          input = { kind = "journal", path = "../evidence/dashboard-repositories/github-pass.json" },
        },
        {
          id = "fail",
          label = "Retained genuine assertion failure",
          source = "Retained GitHub run for onixresearch/kiln; duplicate delivery retained the same execution",
          revision = "d363f0f6f302da6cde1aa5fb66b64c34ec0cc247",
          input = { kind = "journal", path = "../evidence/dashboard-repositories/github-fail.json" },
        },
        {
          id = "recovery",
          label = "Retained controller/bridge restart recovery",
          source = "Retained GitHub run for onixresearch/kiln; executor survived controller/bridge interruption",
          revision = "2bba857fe78760ef6409da4188125dd0bba9a907",
          input = { kind = "journal", path = "../evidence/dashboard-repositories/github-recovery.json" },
        },
        {
          id = "unknown",
          label = "Retained earlier execution: unresolved",
          source = "Retained GitHub run for onixresearch/kiln; Unknown is not a terminal success or failure",
          revision = "19b4f0ef56385d3566b5ade94217d683bae84bf9",
          input = { kind = "journal", path = "../evidence/dashboard-repositories/github-unknown.json" },
        },
      ],
    },
    {
      id = "kiln-radicle",
      label = "Kiln / Radicle self-CI history",
      histories = [
        {
          id = "success",
          label = "Retained real Kiln Nix build: passed",
          source = "Retained dedicated Kiln self-CI fork rad:z2J2jH8gk8G8pzjuvvAsYc6jjyih; not canonical publication",
          revision = "4a2a3e6a80fd5c8c5f42e4a0ace6c333d26d4e8a",
          input = { kind = "journal", path = "../.cairn/archive/1970-01-01-qualify-kiln-self-ci/evidence/success/success-journal.json" },
        },
        {
          id = "failure",
          label = "Retained genuine Kiln Nix build failure",
          source = "Retained dedicated Kiln self-CI fork rad:z2J2jH8gk8G8pzjuvvAsYc6jjyih",
          revision = "86e35b5e9d191f9992fbc3c5fb85a8c0bf51db16",
          input = { kind = "journal", path = "../.cairn/archive/1970-01-01-qualify-kiln-self-ci/evidence/success/failure-journal.json" },
        },
        {
          id = "unavailable",
          label = "Retained unavailable worker: unresolved",
          source = "Retained dedicated Kiln self-CI fork rad:z2J2jH8gk8G8pzjuvvAsYc6jjyih; no successful completion inferred",
          revision = "995da1ff126ece7bbca91f5fc014bf100e4818f7",
          input = { kind = "journal", path = "../.cairn/archive/1970-01-01-qualify-kiln-self-ci/evidence/success/unavailable-journal.json" },
        },
      ],
    },
  ],
}
