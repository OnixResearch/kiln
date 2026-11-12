use kiln_core::types::ExternalRef;
use kiln_provider_api::PortFailure;
use molten::chunk_store::read_manifest_with_root;
use molten::content_store_adapter::{
    ContentOperation, LiveIrohIdentity, assemble_verified_content, execute_live_iroh_stream_get,
    manifest_descriptor, publish_live_iroh_chunks,
};

use super::MoltenContentPort;
use super::support::{domain, exceeds_bound, infrastructure, molten_failure};

impl MoltenContentPort {
    /// Publishes the port's exact manifest over a live Iroh blobs provider.
    pub async fn publish_live(
        &mut self,
        reference: &ExternalRef,
        identity: LiveIrohIdentity<'_>,
    ) -> Result<(), PortFailure> {
        Self::verify_reference_shape(reference)?;
        let digest = reference.native_id.as_str();
        let manifest_ref = self.index.get(digest).ok_or_else(|| {
            infrastructure(
                "aspen_content_missing",
                "Aspen content object is not available",
            )
        })?;
        let stored = read_manifest_with_root(&self.root, manifest_ref)
            .map_err(molten_failure("aspen_content_manifest"))?;
        let manifest = manifest_descriptor(&stored);
        let publication = publish_live_iroh_chunks(
            &self.live_profile,
            &self.root,
            &manifest.manifest_ref,
            identity,
        )
        .await
        .map_err(molten_failure("aspen_content_live_publish"))?;
        self.live = Some(publication);
        Ok(())
    }

    /// Reads the currently published live manifest back through Iroh.
    pub async fn get_live_exact(
        &self,
        reference: &ExternalRef,
        maximum_bytes: u64,
    ) -> Result<Vec<u8>, PortFailure> {
        Self::verify_reference_shape(reference)?;
        if maximum_bytes == 0 {
            return Err(domain(
                "aspen_content_zero_bound",
                "aspen.content.maximum_bytes",
                "content read bound must be nonzero",
            ));
        }
        let publication = self.live.as_ref().ok_or_else(|| {
            infrastructure(
                "aspen_content_live_missing",
                "Aspen live content publication is not available",
            )
        })?;
        let manifest = publication.manifest();
        if manifest.total_length > maximum_bytes {
            return Err(domain(
                "aspen_content_bound",
                "aspen.content.maximum_bytes",
                "Aspen content object exceeds the admitted read bound",
            ));
        }
        let digest = reference.native_id.as_str();
        let command =
            self.command_for(&self.live_profile, ContentOperation::Get, manifest, digest)?;
        let execution = execute_live_iroh_stream_get(
            &self.live_profile,
            publication,
            &command,
            1,
            None,
            std::time::Duration::from_secs(10),
        )
        .await
        .map_err(molten_failure("aspen_content_live_get"))?;
        let bytes = assemble_verified_content(
            manifest,
            &execution.state.artifact,
            &execution.verified_chunks,
        )
        .map_err(molten_failure("aspen_content_live_assemble"))?;
        if exceeds_bound(bytes.len(), maximum_bytes) {
            return Err(domain(
                "aspen_content_bound",
                "aspen.content.maximum_bytes",
                "Aspen content object exceeds the admitted read bound",
            ));
        }
        let observed = blake3::hash(&bytes).to_hex().to_string();
        if observed.as_str() != digest {
            return Err(infrastructure(
                "aspen_content_corrupt",
                "Aspen content bytes fail exact BLAKE3 verification",
            ));
        }
        Ok(bytes)
    }
}
