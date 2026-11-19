let Contracts = import "contracts.ncl" in
({
  schema = Contracts.identity.policy_schema,
  policy_revision = "reviewed-2026-08-08",
  subjects = [
    {
      subject_id = "subject:operator",
      display_label = "Operator",
      authenticators = [
        {
          binding_id = "binding:operator-retired",
          public_key = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
          state = "retired",
        },
        {
          binding_id = "binding:operator-current",
          public_key = "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",
          state = "active",
        },
      ],
    },
  ],
  non_claims = Contracts.required_non_claims,
}) | Contracts.Policy
