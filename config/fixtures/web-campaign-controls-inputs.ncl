{
  campaign_database = "/var/lib/kiln/implementation.sqlite",
  grants = [
    {
      principal = "local:operator",
      campaigns = ["selected-cairn"],
      commands = ["start", "continue", "pause", "cancel", "issue_decision", "resolve_decision"],
      review_roles = ["maintainer"],
    },
    {
      principal = "local:reviewer",
      campaigns = ["selected-cairn"],
      commands = ["resolve_decision"],
      review_roles = ["maintainer"],
      not_after_ms = 1900000000000,
    },
  ],
  definitions = [
    { contract = "/etc/kiln/campaigns/selected-cairn.json", delivery_route = "native_git" },
  ],
}
