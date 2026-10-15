let Base = import "../default.ncl" in
let ExceptOutcome = fun values => std.array.filter (fun value => value != "outcome-machine") values in
Base & {
  schemas | force = std.array.filter (fun schema => schema.id != "outcome-machine") Base.schemas,
  workflow_profile_policy | force = Base.workflow_profile_policy & {
    allowed_profiles | force = ExceptOutcome Base.workflow_profile_policy.allowed_profiles,
  },
  agent_guidance_policy | force = Base.agent_guidance_policy & {
    known_profiles | force = ExceptOutcome Base.agent_guidance_policy.known_profiles,
    known_artifacts | force = ExceptOutcome Base.agent_guidance_policy.known_artifacts,
  },
}
