use durable_file_publication::core::ReplacementMode;
use durable_file_publication::shell::{
    LinuxDirectory, MechanicalIoFailure, PublicationIo, RenameDisposition, StageCreateDisposition,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum IndexFailure {
    Create,
    Write,
    PayloadSync,
    Rename,
    ParentSync,
}

pub(super) struct FailingPublication {
    directory: LinuxDirectory,
    failure: Option<IndexFailure>,
}

impl std::fmt::Debug for FailingPublication {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FailingPublication")
            .field("failure", &self.failure)
            .finish_non_exhaustive()
    }
}

impl FailingPublication {
    pub(super) fn new(directory: LinuxDirectory, failure: Option<IndexFailure>) -> Self {
        Self { directory, failure }
    }

    fn admit(&self, phase: IndexFailure) -> Result<(), MechanicalIoFailure> {
        if self.failure == Some(phase) {
            Err(MechanicalIoFailure)
        } else {
            Ok(())
        }
    }
}

impl PublicationIo for FailingPublication {
    fn supports_no_replace(&self) -> bool {
        self.directory.supports_no_replace()
    }

    fn supports_parent_sync(&self) -> bool {
        self.directory.supports_parent_sync()
    }

    fn create_stage(
        &mut self,
        leaf: &str,
        private_mode: u32,
    ) -> Result<StageCreateDisposition, MechanicalIoFailure> {
        self.admit(IndexFailure::Create)?;
        self.directory.create_stage(leaf, private_mode)
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), MechanicalIoFailure> {
        self.admit(IndexFailure::Write)?;
        self.directory.write_all(bytes)
    }

    fn apply_permissions(&mut self, mode: u32) -> Result<(), MechanicalIoFailure> {
        self.directory.apply_permissions(mode)
    }

    fn sync_payload(&mut self) -> Result<(), MechanicalIoFailure> {
        self.admit(IndexFailure::PayloadSync)?;
        self.directory.sync_payload()
    }

    fn rename_stage(
        &mut self,
        stage_leaf: &str,
        destination_leaf: &str,
        replacement: ReplacementMode,
    ) -> Result<RenameDisposition, MechanicalIoFailure> {
        self.admit(IndexFailure::Rename)?;
        self.directory
            .rename_stage(stage_leaf, destination_leaf, replacement)
    }

    fn sync_parent(&mut self) -> Result<(), MechanicalIoFailure> {
        self.admit(IndexFailure::ParentSync)?;
        self.directory.sync_parent()
    }

    fn cleanup_stage(&mut self, stage_leaf: &str) -> Result<(), MechanicalIoFailure> {
        self.directory.cleanup_stage(stage_leaf)
    }
}
