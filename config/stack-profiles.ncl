let NonEmpty = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value > 0
) in
let Profile = {
  id | NonEmpty,
  role | NonEmpty,
  source_revision | NonEmpty,
  capabilities | Array NonEmpty,
  unsupported_capabilities | Array NonEmpty,
  authority_boundary | NonEmpty,
} in
{
  profiles | Array Profile = [
    {
      id = "mantle-build-v1",
      role = "build",
      source_revision = "0bbab77e23ab95caa4e4184b9945cfaa5c8040e2",
      capabilities = ["dispatch", "observe", "cancel", "read_logs", "reconcile", "products", "metrics"],
      unsupported_capabilities = ["merge", "release"],
      authority_boundary = "Mantle owns build and cache observations; Kiln owns CI state only",
    },
    {
      id = "lattice-workflow-v1",
      role = "workflow",
      source_revision = "5c9902f777412296c13e2347e45b3bede335b828",
      capabilities = ["dispatch", "completion", "cancel"],
      unsupported_capabilities = ["build_truth", "merge"],
      authority_boundary = "Lattice owns workflow execution; Kiln owns CI state only",
    },
    {
      id = "aspen-runtime-v1",
      role = "runtime",
      source_revision = "08e4dfa5443737c81577f6691c673ab1ad78db4d",
      capabilities = ["service_hosting"],
      unsupported_capabilities = ["ci_state", "release"],
      authority_boundary = "Aspen owns runtime transport and persistence; Kiln owns CI state only",
    },
    {
      id = "valence-evidence-v1",
      role = "evidence",
      source_revision = "b37177bbdd9ca0665412e6600f401e5cb54908d2",
      capabilities = ["projection"],
      unsupported_capabilities = ["behavioral_proof", "release"],
      authority_boundary = "Valence owns evidence identity and linkage; Kiln owns CI state only",
    },
    {
      id = "cairn-gate-v1",
      role = "gate",
      source_revision = "15f00875562025e7ea7e0d1f4af24d1a2e2ac06f",
      capabilities = ["evaluate"],
      unsupported_capabilities = ["ci_state", "merge", "release_execution"],
      authority_boundary = "Cairn owns lifecycle gate meaning; Kiln owns candidate identity, process policy, gate composition, and CI state", 
    },
    {
      id = "choregraph-intake-v1",
      role = "machine_intake",
      source_revision = "2afbafb266c60530ae05a6862eb06ace72ae787a",
      capabilities = ["validate_machine"],
      unsupported_capabilities = ["execute", "authorize"],
      authority_boundary = "Choregraph owns graph decisions; provider shells own execution",
    },
  ],
}
