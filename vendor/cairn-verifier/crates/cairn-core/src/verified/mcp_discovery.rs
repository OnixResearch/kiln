//! Pure admission and projection of the existing MCP descriptor authority.
use std::{collections::BTreeSet, fmt};

use super::evidence::canonical_json_hash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const DISCOVERY_SCHEMA: &str = "cairn.mcp-discovery.v1";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoveryLimits {
    pub descriptor_bytes: usize,
    pub page_bytes: usize,
    pub page_entries: usize,
    pub cursor_bytes: usize,
    pub tool_name_bytes: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ToolMembership {
    pub name: String,
    pub requires_mutations: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ToolProfile {
    pub name: String,
    pub tools: Vec<ToolMembership>,
    pub required_routes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoveryPolicy {
    pub schema_version: String,
    pub limits: DiscoveryLimits,
    pub profiles: Vec<ToolProfile>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DiscoveryError {
    InvalidPolicy(String),
    UnsupportedProfile(String),
    Unavailable(String),
    InvalidRequest(String),
    InvalidCursor,
    StaleCursor,
}

impl fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPolicy(message) => write!(f, "invalid discovery policy: {message}"),
            Self::UnsupportedProfile(name) => write!(f, "unsupported tool profile: {name}"),
            Self::Unavailable(name) => write!(f, "tool_unavailable: {name}"),
            Self::InvalidRequest(message) => write!(f, "invalid tools/list request: {message}"),
            Self::InvalidCursor => f.write_str("invalid tools/list cursor"),
            Self::StaleCursor => f.write_str("stale tools/list cursor"),
        }
    }
}

impl std::error::Error for DiscoveryError {}

/// A shell supplies these from its existing descriptor and dispatch authority.
pub struct ToolDescriptor {
    pub descriptor: Value,
    pub requires_mutations: bool,
}

pub struct ToolCatalog {
    tools: Vec<Value>,
    descriptor_sizes: Vec<usize>,
    pub identity: String,
    pub profile: String,
    pub allow_mutations: bool,
    limits: DiscoveryLimits,
}

impl DiscoveryPolicy {
    pub fn parse(source: &str) -> Result<Self, DiscoveryError> {
        serde_json::from_str(source)
            .map_err(|error| DiscoveryError::InvalidPolicy(error.to_string()))
    }

    // r[impl cairn.mcp_discovery.profiles]
    pub fn admit(
        &self,
        authority: &[ToolDescriptor],
        profile: &str,
        allow_mutations: bool,
    ) -> Result<ToolCatalog, DiscoveryError> {
        let invalid = |message: &str| DiscoveryError::InvalidPolicy(message.into());
        let limits = &self.limits;
        if self.schema_version != DISCOVERY_SCHEMA
            || limits.descriptor_bytes == 0
            || limits.page_entries == 0
            || limits.tool_name_bytes == 0
            || limits.cursor_bytes < 88
            || limits.page_bytes
                < limits
                    .descriptor_bytes
                    .saturating_add(limits.cursor_bytes)
                    .saturating_add(40)
        {
            return Err(invalid("unsupported schema or inconsistent bounds"));
        }
        let mut names = BTreeSet::new();
        let mut sizes = Vec::with_capacity(authority.len());
        for tool in authority {
            let name = tool.descriptor["name"]
                .as_str()
                .ok_or_else(|| invalid("descriptor name missing"))?;
            if name.is_empty() || name.len() > limits.tool_name_bytes || !names.insert(name) {
                return Err(invalid("invalid or duplicate descriptor name"));
            }
            if tool.descriptor["annotations"]["readOnlyHint"].as_bool()
                != Some(!tool.requires_mutations)
                || tool.descriptor["annotations"]["destructiveHint"].as_bool()
                    != Some(tool.requires_mutations)
                || !tool.descriptor["inputSchema"].is_object()
            {
                return Err(invalid("descriptor capability or input schema mismatch"));
            }
            let size = serde_json::to_vec(&tool.descriptor)
                .map_err(|error| invalid(&error.to_string()))?
                .len();
            if size > limits.descriptor_bytes {
                return Err(invalid("descriptor exceeds descriptor_bytes"));
            }
            sizes.push(size);
        }
        let supported = ["full", "inspect", "author", "verify"];
        let mut profile_names = BTreeSet::new();
        for candidate in &self.profiles {
            if !supported.contains(&candidate.name.as_str())
                || !profile_names.insert(candidate.name.as_str())
            {
                return Err(invalid("unknown or duplicate profile"));
            }
            let mut members = BTreeSet::new();
            for member in &candidate.tools {
                let tool = authority
                    .iter()
                    .find(|tool| tool.descriptor["name"].as_str() == Some(&member.name))
                    .ok_or_else(|| invalid("unknown profile tool"))?;
                if !members.insert(member.name.as_str())
                    || member.requires_mutations != tool.requires_mutations
                {
                    return Err(invalid(
                        "duplicate tool or inconsistent capability declaration",
                    ));
                }
            }
            if !members.contains("cairn_tool_describe") || candidate.required_routes.is_empty() {
                return Err(invalid("profile requires describe and diagnostic routes"));
            }
            let mut routes = BTreeSet::new();
            for route in &candidate.required_routes {
                if !routes.insert(route)
                    || !members.contains(route.as_str())
                    || !candidate
                        .tools
                        .iter()
                        .any(|tool| tool.name == *route && !tool.requires_mutations)
                {
                    return Err(invalid("required routes must be unique read-only members"));
                }
            }
            if candidate.name == "full" && members != names {
                return Err(invalid("full profile must cover the descriptor authority"));
            }
        }
        if profile_names.len() != supported.len() {
            return Err(invalid("all supported profiles are required"));
        }
        let selected = self
            .profiles
            .iter()
            .find(|candidate| candidate.name == profile)
            .ok_or_else(|| DiscoveryError::UnsupportedProfile(profile.into()))?;
        let mut tools = Vec::with_capacity(selected.tools.len());
        let mut descriptor_sizes = Vec::with_capacity(selected.tools.len());
        for member in &selected.tools {
            if member.requires_mutations && !allow_mutations {
                continue;
            }
            let index = authority
                .iter()
                .position(|tool| tool.descriptor["name"].as_str() == Some(&member.name))
                .ok_or_else(|| invalid("unknown selected tool"))?;
            tools.push(authority[index].descriptor.clone());
            descriptor_sizes.push(sizes[index]);
        }
        let identity = canonical_json_hash(&json!({
            "schema_version": DISCOVERY_SCHEMA,
            "policy": self,
            "authority": authority.iter().map(|tool| &tool.descriptor).collect::<Vec<_>>(),
            "profile": profile,
            "allow_mutations": allow_mutations,
            "tools": tools,
        }));
        Ok(ToolCatalog {
            tools,
            descriptor_sizes,
            identity,
            profile: profile.into(),
            allow_mutations,
            limits: limits.clone(),
        })
    }
}

impl ToolCatalog {
    pub fn tools(&self) -> &[Value] {
        &self.tools
    }

    // r[impl cairn.mcp_discovery.lookup]
    pub fn describe(&self, name: &str) -> Result<&Value, DiscoveryError> {
        if name.is_empty() || name.len() > self.limits.tool_name_bytes {
            return Err(DiscoveryError::Unavailable(name.into()));
        }
        self.tools
            .iter()
            .find(|tool| tool["name"].as_str() == Some(name))
            .ok_or_else(|| DiscoveryError::Unavailable(name.into()))
    }

    // r[impl cairn.mcp_discovery.pages]
    pub fn list(&self, params: Option<&Value>, paged: bool) -> Result<Value, DiscoveryError> {
        let cursor = match params {
            None => None,
            Some(value) => {
                let object = value
                    .as_object()
                    .ok_or_else(|| DiscoveryError::InvalidRequest("expected object".into()))?;
                if object.keys().any(|key| key != "cursor") {
                    return Err(DiscoveryError::InvalidRequest("unknown field".into()));
                }
                match object.get("cursor") {
                    None => None,
                    Some(value) => {
                        let cursor = value.as_str().ok_or_else(|| {
                            DiscoveryError::InvalidRequest("expected string cursor".into())
                        })?;
                        if cursor.len() > self.limits.cursor_bytes {
                            return Err(DiscoveryError::InvalidCursor);
                        }
                        Some(cursor)
                    }
                }
            }
        };
        if !paged {
            if cursor.is_some() {
                return Err(DiscoveryError::InvalidRequest(
                    "pagination is not enabled".into(),
                ));
            }
            return Ok(json!({"tools": self.tools}));
        }
        let start = match cursor {
            None => 0,
            Some(cursor) => {
                let mut fields = cursor.split(':');
                if fields.next() != Some("v1") {
                    return Err(DiscoveryError::InvalidCursor);
                }
                let identity = fields.next().ok_or(DiscoveryError::InvalidCursor)?;
                let position = fields.next().ok_or(DiscoveryError::InvalidCursor)?;
                if fields.next().is_some()
                    || identity.len() != 64
                    || !identity
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                {
                    return Err(DiscoveryError::InvalidCursor);
                }
                let index: usize = position
                    .parse()
                    .map_err(|_| DiscoveryError::InvalidCursor)?;
                if index.to_string() != position || index == 0 {
                    return Err(DiscoveryError::InvalidCursor);
                }
                if identity != self.identity {
                    return Err(DiscoveryError::StaleCursor);
                }
                if index >= self.tools.len() {
                    return Err(DiscoveryError::InvalidCursor);
                }
                index
            }
        };
        let mut end = start;
        let mut bytes = 0usize;
        while end < self.tools.len() && end - start < self.limits.page_entries {
            let next = end + 1;
            let descriptor_bytes = bytes + self.descriptor_sizes[end] + usize::from(end > start);
            let continuation_bytes = if next < self.tools.len() {
                // ,"nextCursor":"v1:<identity>:<position>"
                16 + 3 + self.identity.len() + 1 + next.to_string().len()
            } else {
                0
            };
            if 12 + descriptor_bytes + continuation_bytes > self.limits.page_bytes {
                break;
            }
            bytes = descriptor_bytes;
            end = next;
        }
        if end == start && start < self.tools.len() {
            return Err(DiscoveryError::InvalidPolicy(
                "page cannot fit a descriptor".into(),
            ));
        }
        let mut result = json!({"tools": &self.tools[start..end]});
        if end < self.tools.len() {
            result["nextCursor"] = json!(format!("v1:{}:{end}", self.identity));
        }
        Ok(result)
    }
}

#[cfg(test)]
#[path = "mcp_discovery_tests.rs"]
mod tests;
