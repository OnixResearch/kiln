# Synthetic immutable paths for admission conformance, not realized deployment tools.
{
  root = "/var/tmp/kiln-radicle-live-fixture",
  source_branch = "ci-canary",
  status_checkpoint_branch = "kiln-status-checkpoint",
  tools = {
    radicle = "/nix/store/00000000000000000000000000000000-radicle-fixture/bin/rad",
    radicle_node = "/nix/store/00000000000000000000000000000000-radicle-fixture/bin/radicle-node",
    broker = "/nix/store/00000000000000000000000000000000-broker-fixture/bin/radicle-ci-broker",
    broker_tool = "/nix/store/00000000000000000000000000000000-broker-fixture/bin/cibtool",
    jj = "/nix/store/00000000000000000000000000000000-jj-fixture/bin/jj",
    git = "/nix/store/00000000000000000000000000000000-git-fixture/bin/git",
    secretspec = (import "../radicle-status-profile.ncl").profile.signer.executable_path,
  },
}
