# Negative fixture: the maintained Radicle workflow cannot execute resume.
let shell = import "../radicle-workflow-shell.ncl" in
shell & { profile.workflow.controls.resume | force = true }
