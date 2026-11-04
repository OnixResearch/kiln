(import "../patchgate-review-job.ncl") {
  node_id = "review",
  python = "/nix/store/00000000000000000000000000000000-python3/bin/python3",
  script = "scripts/patchgate-review-job.py",
  target = "/srv/kiln/patchgate-review-target",
  adapter = "/nix/store/00000000000000000000000000000000-kiln/bin/kiln-patchgate-review",
  patchgate = "/nix/store/00000000000000000000000000000000-patchgate/bin/patchgate",
  pijul = "/nix/store/00000000000000000000000000000000-pijul/bin/pijul",
  home = "/var/lib/kiln/patchgate-review-home",
  pipeline = "patchgate-review-v1",
  output = "patchgate-review.json",
  path = "",
}
