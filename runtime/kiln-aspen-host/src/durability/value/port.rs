use molten::system_extension::{
    NativeCallbackValue, NativeCallbackValuePort, NativeValuePortFailure,
    NativeValuePortFailureKind, NativeValuePublicationReceipt, admit_native_callback_value,
};

use super::DurableCallbackValuePort;
use super::io::{failure, leaf_from_value_ref};

impl NativeCallbackValuePort for DurableCallbackValuePort {
    fn materialize(
        &mut self,
        value_ref: &str,
        maximum_bytes: u64,
    ) -> Result<NativeCallbackValue, NativeValuePortFailure> {
        let admitted_maximum = maximum_bytes.min(self.limits.maximum_value_bytes);
        let leaf = leaf_from_value_ref(value_ref)?;
        self.read_value(&leaf, value_ref, admitted_maximum)
    }

    fn publish(
        &mut self,
        value: &NativeCallbackValue,
        maximum_bytes: u64,
    ) -> Result<NativeValuePublicationReceipt, NativeValuePortFailure> {
        admit_native_callback_value(value, maximum_bytes)?;
        if maximum_bytes > self.limits.maximum_value_bytes {
            return Err(failure(
                NativeValuePortFailureKind::BoundExceeded,
                "requested callback publication bound exceeds the durable profile",
            ));
        }
        self.publish_value(value)
    }
}
