use reflexion::{ffi_box::FfiBox, ffi_slice::FfiSlice};
use registry_ffi::{PluginHandle, PluginOpaque, QueryHandle, QueryIndex};

/// A Plugin correspond to an "endpoint"
/// it contain both cache for the endpoint, and every related system
struct Plugin {
    systems: Vec<System>,
    cache: FfiBox<PluginOpaque>,
}

/// A system gather a set of queries, and matching execution function
pub struct System {
    queries: Vec<QueryIndex>,
    executor: extern "C" fn(PluginHandle, FfiSlice<QueryHandle>),
}
