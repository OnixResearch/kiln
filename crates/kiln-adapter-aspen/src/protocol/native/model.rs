#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AspenMaterializedValue {
    pub value_ref: String,
    pub bytes: Vec<u8>,
}

impl AspenMaterializedValue {
    pub fn exact(bytes: Vec<u8>) -> Self {
        Self {
            value_ref: super::aspen_content_ref(&bytes),
            bytes,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AspenNativeCallbackEnvelope {
    pub manifest_ref: String,
    pub executable_ref: String,
    pub instance_id: String,
    pub extension_id: String,
    pub service_id: String,
    pub generation: u64,
    pub callback: String,
    pub sequence: u64,
    pub event_ref: String,
    pub payload: Option<AspenMaterializedValue>,
    pub logical_tick: u64,
    pub deadline_tick: u64,
    pub state: Option<AspenMaterializedValue>,
    pub policy_refs: Vec<String>,
    pub resource_ref: String,
    pub port_binding_refs: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AspenNativeEffect {
    pub port_id: String,
    pub port_version: String,
    pub operation: String,
    pub input_schema: String,
    pub output_schema: String,
    pub request: AspenMaterializedValue,
    pub generation: u64,
    pub accounted_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AspenEffectCompletion {
    pub request_ref: String,
    pub generation: u64,
    pub output_ref: String,
    pub output: Option<AspenMaterializedValue>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AspenNativeCallbackOutcome {
    pub outputs: Vec<AspenMaterializedValue>,
    pub effects: Vec<AspenNativeEffect>,
    pub state: Option<AspenMaterializedValue>,
    pub checkpoint: Option<AspenMaterializedValue>,
    pub health: String,
}
