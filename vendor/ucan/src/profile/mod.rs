/// Root shell profile modules. The wire and authorization profiles live in
/// the `v1` and `v1_authorization` submodules, with test field builders in
/// `wire_fields`.
pub mod v1;
pub mod v1_authorization;
#[cfg(test)]
pub(crate) mod wire_fields;
