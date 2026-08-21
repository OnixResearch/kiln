let shell = import "../radicle-workflow-shell.ncl" in
shell & {
  profile.maximum_replay_records = 0,
}
