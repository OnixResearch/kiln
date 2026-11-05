# Negative fixture: the maintained Radicle workflow cannot execute pause.
let shell = import "../radicle-workflow-shell.ncl" in
shell & { profile.workflow.controls.pause | force = true }
