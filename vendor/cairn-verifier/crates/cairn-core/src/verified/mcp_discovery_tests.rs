use super::*;

fn policy_and_authority() -> (DiscoveryPolicy, Vec<ToolDescriptor>) {
    let policy = DiscoveryPolicy::parse(include_str!(
        "../../../../cairn-policy/generated/mcp-discovery.json"
    ))
    .expect("generated discovery policy");
    let full = policy
        .profiles
        .iter()
        .find(|profile| profile.name == "full")
        .expect("full");
    let authority = full.tools.iter().map(|member| ToolDescriptor {
        descriptor: json!({
            "name":member.name,
            "description":"bounded test authority",
            "inputSchema":{"type":"object", "additionalProperties":false, "properties":{}},
            "annotations":{"readOnlyHint":!member.requires_mutations, "destructiveHint":member.requires_mutations}
        }),
        requires_mutations: member.requires_mutations,
    }).collect();
    (policy, authority)
}

#[test]
fn policy_admission_rejects_invalid_membership_capabilities_and_missing_routes() {
    // r[verify cairn.mcp_discovery.profiles]
    let (policy, authority) = policy_and_authority();
    assert!(policy.admit(&authority, "full", false).is_ok());
    assert!(matches!(
        policy.admit(&authority, "unknown", false),
        Err(DiscoveryError::UnsupportedProfile(_))
    ));
    for change in [
        "duplicate-profile",
        "unsupported-profile",
        "missing-profile",
        "duplicate-tool",
        "unknown-tool",
        "capability",
        "missing-route",
        "mutating-route",
        "missing-describe",
        "incomplete-full",
        "zero-bound",
        "inconsistent-bound",
    ] {
        let mut invalid = policy.clone();
        match change {
            "duplicate-profile" => invalid.profiles.push(invalid.profiles[0].clone()),
            "unsupported-profile" => invalid.profiles[0].name = "other".into(),
            "missing-profile" => {
                invalid.profiles.pop();
            }
            "duplicate-tool" => {
                let duplicate = invalid.profiles[0].tools[0].clone();
                invalid.profiles[0].tools.push(duplicate);
            }
            "unknown-tool" => invalid.profiles[0].tools[0].name = "not-a-tool".into(),
            "capability" => invalid.profiles[0].tools[0].requires_mutations = true,
            "missing-route" => invalid.profiles[0]
                .required_routes
                .push("not-a-route".into()),
            "mutating-route" => invalid.profiles[0]
                .required_routes
                .push("cairn_init".into()),
            "missing-describe" => invalid.profiles[0]
                .tools
                .retain(|tool| tool.name != "cairn_tool_describe"),
            "incomplete-full" => invalid.profiles[0]
                .tools
                .retain(|tool| tool.name != "cairn_status"),
            "zero-bound" => invalid.limits.page_entries = 0,
            "inconsistent-bound" => invalid.limits.page_bytes = 1,
            _ => unreachable!(),
        }
        assert!(
            matches!(
                invalid.admit(&authority, "inspect", false),
                Err(DiscoveryError::InvalidPolicy(_))
            ),
            "{change}"
        );
    }
    let mut authority = authority;
    authority[0].descriptor["annotations"]["readOnlyHint"] = json!(false);
    assert!(matches!(
        policy.admit(&authority, "full", false),
        Err(DiscoveryError::InvalidPolicy(_))
    ));
    let mut value = serde_json::to_value(&policy).expect("policy value");
    value["unknown"] = json!(true);
    assert!(DiscoveryPolicy::parse(&value.to_string()).is_err());
}

#[test]
fn schema_policy_and_order_drift_invalidate_continuations() {
    // r[verify cairn.mcp_discovery.pages]
    let (policy, mut authority) = policy_and_authority();
    let original = policy.admit(&authority, "full", true).expect("catalog");
    let first = original.list(None, true).expect("page");
    let params = json!({"cursor":first["nextCursor"]});
    assert!(original.list(Some(&params), true).is_ok());
    authority[0].descriptor["inputSchema"]["properties"]["new_argument"] = json!({"type":"string"});
    let changed_schema = policy
        .admit(&authority, "full", true)
        .expect("schema catalog");
    assert!(matches!(
        changed_schema.list(Some(&params), true),
        Err(DiscoveryError::StaleCursor)
    ));
    let schema_page = changed_schema.list(None, true).expect("schema page");
    let schema_params = json!({"cursor":schema_page["nextCursor"]});
    let mut changed_policy = policy.clone();
    changed_policy.limits.page_entries += 1;
    let changed = changed_policy
        .admit(&authority, "full", true)
        .expect("policy catalog");
    assert!(matches!(
        changed.list(Some(&schema_params), true),
        Err(DiscoveryError::StaleCursor)
    ));
    changed_policy.profiles[0].tools.swap(0, 1);
    let reordered = changed_policy
        .admit(&authority, "full", true)
        .expect("reordered catalog");
    assert_ne!(changed.identity, reordered.identity);
    let changed_page = changed.list(None, true).expect("policy page");
    let policy_params = json!({"cursor":changed_page["nextCursor"]});
    assert!(matches!(
        reordered.list(Some(&policy_params), true),
        Err(DiscoveryError::StaleCursor)
    ));
    assert!(matches!(
        original.describe(&"x".repeat(policy.limits.tool_name_bytes + 1)),
        Err(DiscoveryError::Unavailable(_))
    ));
}

#[test]
fn byte_bound_splits_pages_and_rejects_oversized_descriptors() {
    // r[verify cairn.mcp_discovery.pages]
    let (mut policy, mut authority) = policy_and_authority();
    policy.limits.descriptor_bytes = 512;
    policy.limits.page_bytes = 1000;
    policy.limits.cursor_bytes = 128;
    for tool in &mut authority {
        tool.descriptor["description"] = json!("x".repeat(200));
    }
    let catalog = policy
        .admit(&authority, "full", true)
        .expect("bounded catalog");
    let first = catalog.list(None, true).expect("first page");
    assert!(first["tools"].as_array().expect("tools").len() < policy.limits.page_entries);
    let mut count = 0;
    let mut params = None;
    loop {
        let page = catalog.list(params.as_ref(), true).expect("bounded page");
        assert!(serde_json::to_vec(&page).expect("page bytes").len() <= policy.limits.page_bytes);
        count += page["tools"].as_array().expect("tools").len();
        let Some(cursor) = page["nextCursor"].as_str() else {
            break;
        };
        params = Some(json!({"cursor":cursor}));
    }
    assert_eq!(count, authority.len());
    authority[0].descriptor["description"] = json!("x".repeat(513));
    assert!(matches!(
        policy.admit(&authority, "full", true),
        Err(DiscoveryError::InvalidPolicy(_))
    ));
    assert!(matches!(
        policy.admit(&[], "full", false),
        Err(DiscoveryError::InvalidPolicy(_))
    ));
}
