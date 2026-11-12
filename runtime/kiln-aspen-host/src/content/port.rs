use kiln_adapter_aspen::AspenContentPort;
use kiln_core::types::ExternalRef;
use kiln_provider_api::PortFailure;
use molten::chunk_store::{put_bytes_with_root, read_manifest_with_root};
use molten::content_store_adapter::{
    ContentOperation, assemble_verified_content, execute_local_stream_get,
    execute_local_stream_put, manifest_descriptor,
};

use super::support::{domain, infrastructure, molten_failure, verification_receipt};
use super::{MoltenContentPort, OBJECT_KIND};

impl AspenContentPort for MoltenContentPort {
    // r[impl kiln.aspen_runtime.content] r[impl kiln.aspen_runtime.identity]
    fn put_exact(
        &mut self,
        reference: &ExternalRef,
        bytes: &[u8],
    ) -> Result<ExternalRef, PortFailure> {
        Self::verify_reference_shape(reference)?;
        let digest = blake3::hash(bytes).to_hex().to_string();
        if reference.native_id.as_str() != digest.as_str() {
            return Err(domain(
                "aspen_content_identity",
                "aspen.content.reference",
                "content reference does not match the exact supplied bytes",
            ));
        }
        if let Some(manifest_ref) = self.index.get(&digest) {
            let stored = read_manifest_with_root(&self.root, manifest_ref)
                .map_err(molten_failure("aspen_content_manifest"))?;
            let existing =
                molten::chunk_store::read_object_with_root(&self.root, &stored.manifest_ref)
                    .map_err(molten_failure("aspen_content_read"))?;
            if existing.bytes != bytes {
                return Err(domain(
                    "aspen_content_collision",
                    "aspen.content.reference",
                    "existing content under the same reference has different bytes",
                ));
            }
        } else {
            let put = put_bytes_with_root(&self.root, OBJECT_KIND, bytes, self.chunk_size)
                .map_err(molten_failure("aspen_content_put"))?;
            let stored = read_manifest_with_root(&self.root, &put.manifest_ref)
                .map_err(molten_failure("aspen_content_manifest"))?;
            let manifest = manifest_descriptor(&stored);
            let command = self.command(ContentOperation::Put, &manifest, &digest)?;
            execute_local_stream_put(
                &self.profile,
                &self.root,
                &command,
                &manifest,
                OBJECT_KIND,
                bytes,
            )
            .map_err(molten_failure("aspen_content_put_verified"))?;
            self.index.insert(digest, put.manifest_ref);
            self.persist_index()?;
        }
        verification_receipt(reference, bytes.len())
    }

    // r[impl kiln.aspen_runtime.content]
    fn get_exact(
        &self,
        reference: &ExternalRef,
        maximum_bytes: usize,
    ) -> Result<Vec<u8>, PortFailure> {
        Self::verify_reference_shape(reference)?;
        if maximum_bytes == 0 {
            return Err(domain(
                "aspen_content_zero_bound",
                "aspen.content.maximum_bytes",
                "content read bound must be nonzero",
            ));
        }
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
        if manifest.total_length > maximum_bytes as u64 {
            return Err(domain(
                "aspen_content_bound",
                "aspen.content.maximum_bytes",
                "Aspen content object exceeds the admitted read bound",
            ));
        }
        let command = self.command(ContentOperation::Get, &manifest, digest)?;
        let execution = execute_local_stream_get(&self.profile, &self.root, &command, 1, None)
            .map_err(molten_failure("aspen_content_get_verified"))?;
        let bytes = assemble_verified_content(
            &execution.manifest,
            &execution.state.artifact,
            &execution.verified_chunks,
        )
        .map_err(molten_failure("aspen_content_assemble"))?;
        if bytes.len() > maximum_bytes {
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
