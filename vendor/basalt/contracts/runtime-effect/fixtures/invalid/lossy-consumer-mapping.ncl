let Contracts = import "../../contracts.ncl" in
{
  schema = "basalt.runtime-effect-consumer-mappings.v1",
  animus = { revision = Contracts.identity.animus_revision, source_paths = [], mappings = [] },
  molten = { revision = Contracts.identity.molten_revision, source_paths = [], mappings = [] },
  non_claims = ["mapping is complete"],
} | Contracts.ConsumerMappings
