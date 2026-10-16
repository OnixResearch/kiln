use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use kiln_core::types::{EffectId, ExternalRef};
use kiln_provider_api::{InfrastructureFailure, PortFailure};

use crate::identity::{
    ContentIdentityInput, IdentityMappingReceipt, KilnArtifactRole, admit_mapping,
    map_content_identity,
};
use crate::profile::AdmittedAspenRuntimeProfile;

pub const CONTENT_VERIFICATION_KIND: &str = "content-verification-blake3";
pub const CONTENT_REFERENCE_KIND: &str = "content-blake3";
pub const ASPEN_CONTENT_SCHEMA: &str = "molten.content-store-adapter.event.v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MappedContent {
    pub reference: ExternalRef,
    pub mapping: IdentityMappingReceipt,
}

pub trait AspenContentPort {
    fn put_exact(
        &mut self,
        reference: &ExternalRef,
        bytes: &[u8],
    ) -> Result<ExternalRef, PortFailure>;
    fn get_exact(
        &self,
        reference: &ExternalRef,
        maximum_bytes: usize,
    ) -> Result<Vec<u8>, PortFailure>;
}

#[derive(Clone, Debug, Default)]
pub struct InMemoryAspenContent {
    objects: Arc<Mutex<BTreeMap<ExternalRef, Vec<u8>>>>,
}

impl InMemoryAspenContent {
    pub fn remove(&self, reference: &ExternalRef) -> Result<(), PortFailure> {
        let mut objects = self.objects.lock().map_err(|_| poisoned_failure())?;
        objects.remove(reference);
        Ok(())
    }

    pub fn replace_unchecked(
        &self,
        reference: &ExternalRef,
        bytes: Vec<u8>,
    ) -> Result<(), PortFailure> {
        let mut objects = self.objects.lock().map_err(|_| poisoned_failure())?;
        objects.insert(reference.clone(), bytes);
        Ok(())
    }
}

impl AspenContentPort for InMemoryAspenContent {
    fn put_exact(
        &mut self,
        reference: &ExternalRef,
        bytes: &[u8],
    ) -> Result<ExternalRef, PortFailure> {
        reference.validate().map_err(PortFailure::Domain)?;
        let observed = blake3::hash(bytes).to_hex().to_string();
        if reference.provider_namespace.as_str() != "molten"
            || reference.native_kind.as_str() != CONTENT_REFERENCE_KIND
            || reference.native_id.as_str() != observed
        {
            return Err(PortFailure::Domain(kiln_core::types::Diagnostic::new(
                "aspen_content_identity",
                "aspen.content.reference",
                "content reference does not match the exact supplied bytes",
            )));
        }
        let mut objects = self.objects.lock().map_err(|_| poisoned_failure())?;
        if let Some(existing) = objects.get(reference)
            && existing.as_slice() != bytes
        {
            return Err(PortFailure::Domain(kiln_core::types::Diagnostic::new(
                "aspen_content_collision",
                "aspen.content.reference",
                "existing content under the same reference has different bytes",
            )));
        }
        objects.insert(reference.clone(), bytes.to_vec());
        let verification_digest =
            blake3::hash(format!("{}\0{}", reference.native_id.as_str(), bytes.len()).as_bytes())
                .to_hex()
                .to_string();
        ExternalRef::new("molten", CONTENT_VERIFICATION_KIND, &verification_digest)
            .map_err(PortFailure::Domain)
    }

    fn get_exact(
        &self,
        reference: &ExternalRef,
        maximum_bytes: usize,
    ) -> Result<Vec<u8>, PortFailure> {
        reference.validate().map_err(PortFailure::Domain)?;
        if maximum_bytes == 0 {
            return Err(PortFailure::Domain(kiln_core::types::Diagnostic::new(
                "aspen_content_zero_bound",
                "aspen.content.maximum_bytes",
                "content read bound must be nonzero",
            )));
        }
        let objects = self.objects.lock().map_err(|_| poisoned_failure())?;
        let bytes = objects.get(reference).ok_or_else(|| {
            PortFailure::Infrastructure(infrastructure(
                "aspen_content_missing",
                "Aspen content object is not available",
            ))
        })?;
        if bytes.len() > maximum_bytes {
            return Err(PortFailure::Domain(kiln_core::types::Diagnostic::new(
                "aspen_content_bound",
                "aspen.content.maximum_bytes",
                "Aspen content object exceeds the admitted read bound",
            )));
        }
        let observed = blake3::hash(bytes).to_hex().to_string();
        if reference.provider_namespace.as_str() != "molten"
            || reference.native_kind.as_str() != CONTENT_REFERENCE_KIND
            || reference.native_id.as_str() != observed
        {
            return Err(PortFailure::Infrastructure(infrastructure(
                "aspen_content_corrupt",
                "Aspen content bytes fail exact BLAKE3 verification",
            )));
        }
        Ok(bytes.clone())
    }
}

#[derive(Clone, Debug)]
pub struct AspenContentAdapter<P> {
    admitted: AdmittedAspenRuntimeProfile,
    port: P,
}

impl<P> AspenContentAdapter<P> {
    #[must_use]
    pub const fn new(admitted: AdmittedAspenRuntimeProfile, port: P) -> Self {
        Self { admitted, port }
    }

    #[must_use]
    pub const fn port(&self) -> &P {
        &self.port
    }

    pub const fn port_mut(&mut self) -> &mut P {
        &mut self.port
    }
}

impl<P: AspenContentPort> AspenContentAdapter<P> {
    // r[impl kiln.aspen_runtime.content] r[impl kiln.aspen_runtime.identity]
    pub fn store(
        &mut self,
        role: KilnArtifactRole,
        kiln_schema: &str,
        bytes: &[u8],
        producer_effect: Option<EffectId>,
    ) -> Result<MappedContent, PortFailure> {
        if bytes.len() > self.admitted.profile.bounds.max_content_bytes {
            return Err(PortFailure::Domain(kiln_core::types::Diagnostic::new(
                "aspen_content_bound",
                "aspen.content.bytes",
                "content exceeds the admitted Aspen content bound",
            )));
        }
        if role == KilnArtifactRole::Log && bytes.len() > self.admitted.profile.bounds.max_log_bytes
        {
            return Err(PortFailure::Domain(kiln_core::types::Diagnostic::new(
                "aspen_log_bound",
                "aspen.content.log",
                "log content exceeds the admitted bounded-log profile",
            )));
        }
        let digest = blake3::hash(bytes).to_hex().to_string();
        let reference = ExternalRef::new("molten", CONTENT_REFERENCE_KIND, &digest)
            .map_err(PortFailure::Domain)?;
        let verification_receipt = self.port.put_exact(&reference, bytes)?;
        let mapping = map_content_identity(ContentIdentityInput {
            role,
            kiln_schema,
            aspen_kind: CONTENT_REFERENCE_KIND,
            aspen_schema: ASPEN_CONTENT_SCHEMA,
            bytes,
            producer_effect,
            verification_receipt,
        })
        .map_err(PortFailure::Domain)?;
        Ok(MappedContent { reference, mapping })
    }

    // r[impl kiln.aspen_runtime.content]
    pub fn load(
        &self,
        mapped: &MappedContent,
        maximum_bytes: usize,
    ) -> Result<Vec<u8>, PortFailure> {
        if maximum_bytes == 0 || maximum_bytes > self.admitted.profile.bounds.max_content_bytes {
            return Err(PortFailure::Domain(kiln_core::types::Diagnostic::new(
                "aspen_content_bound",
                "aspen.content.maximum_bytes",
                "content read bound must fit the admitted profile",
            )));
        }
        let bytes = self.port.get_exact(&mapped.reference, maximum_bytes)?;
        admit_mapping(&mapped.mapping, &bytes).map_err(PortFailure::Domain)?;
        if mapped.reference.native_id.as_str() != mapped.mapping.aspen.digest {
            return Err(PortFailure::Domain(kiln_core::types::Diagnostic::new(
                "aspen_content_mapping_drift",
                "aspen.content.mapping",
                "content reference does not match its identity mapping receipt",
            )));
        }
        Ok(bytes)
    }
}

fn infrastructure(code: &str, message: &str) -> InfrastructureFailure {
    InfrastructureFailure {
        code: code.to_string(),
        message: message.to_string(),
    }
}

fn poisoned_failure() -> PortFailure {
    PortFailure::Infrastructure(infrastructure(
        "aspen_content_lock",
        "local Aspen content fixture lock is unavailable",
    ))
}
