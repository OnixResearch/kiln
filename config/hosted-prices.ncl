# Operator-authored admission inputs, not merchant approval or live payment authority.
# Export JSON and admit each exact price through finance.price.admit. Monetary values
# are integer currency minor units; unit_ms is trusted host metering granularity.
let c = import "github-contracts.ncl" in
let Identifier =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 128
    && std.string.is_match "^[A-Za-z0-9_.-]+$" value
  )
in
let Currency =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^[a-z]{3}$" value
  )
in
fun input =>
  let settings =
    input
      | {
        organization | Identifier,
        account
          | {
            request_id | Identifier,
            expected_revision | c.BoundedInteger 0 4294967295,
            currency | Currency,
            cap_minor | c.BoundedInteger 1 1000000000000,
            request_max_minor | c.BoundedInteger 1 1000000000000,
            max_concurrent | c.BoundedInteger 1 1024,
          },
        prices
          | Array {
            class | Identifier,
            currency | Currency,
            unit_minor | c.BoundedInteger 1 1000000000000,
            unit_ms | c.BoundedInteger 1 86400000,
            max_units | c.BoundedInteger 1 1000000000,
          },
        sandbox
          | {
            enabled | Bool | default = false,
            endpoint | std.contract.from_predicate (fun value => value == "https://api.stripe.com/v1"),
            stripe_version | std.contract.from_predicate (fun value => value == "2024-06-20"),
            secret_key_file | c.AbsolutePath,
            webhook_secret_file | c.AbsolutePath,
            customer_id
              | std.contract.from_predicate (fun value =>
                std.is_string value
                && std.string.length value <= 128
                && std.string.is_match "^cus_[A-Za-z0-9]+$" value
              ),
          },
      }
  in
  {
    schema = "kiln.hosted.prices.v1",
    organization = settings.organization,
    account = settings.account,
    prices = settings.prices,
    sandbox = settings.sandbox,
  }
