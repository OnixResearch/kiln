let shell = import "../radicle-workflow-shell.ncl" in
shell & {
  profile.capabilities.collaboration = true,
}
