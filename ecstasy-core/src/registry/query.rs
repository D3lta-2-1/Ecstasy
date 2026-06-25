use super::{ArchetypeIndex, ColumnIndex, archetype_manager::ArchetypeManager};
use ecstasy_ffi::{self, Component, LocalColumnIndex, RegistryError, TypeIdentity};
use reflexion::{ffi_enum::FfiResult, ffi_slice::FfiSlice};
use std::collections::HashMap;

/// a QuerySet is a shortcut to access archetypes, being part of the registry storage, that share common properties
/// a QuerySet is able to quickly find all column that contain sized component
/// it gather all archetype that share a common set of components, but doesn't care about mutability
pub struct QuerySet {
    pub(crate) requested_components: Vec<Component>, // contain all components, even unsized components
    pub(crate) accessible_components: HashMap<TypeIdentity, LocalColumnIndex>, // only keep sized components, should I store the "associated components instead ?"
    pub(crate) archetypes: HashMap<ArchetypeIndex, Vec<ColumnIndex>>, //where stuff is located in the archetype.
}

impl QuerySet {
    pub fn requested_components(&self) -> &[Component] {
        &self.requested_components
    }

    pub fn add_archetype(
        &mut self,
        archetype_index: ArchetypeIndex,
        archetypes: &ArchetypeManager,
    ) {
        let mut mapping = vec![ColumnIndex(0); self.accessible_components.len()];
        for (component, local_index) in self.accessible_components.iter() {
            let component = archetypes
                .find_component(component)
                .expect("this archetype don't match the query requirements");
            let column = archetypes.find_column(component, archetype_index);
            mapping[local_index.0] = column;
        }
        self.archetypes.insert(archetype_index, mapping);
    }
}

impl ecstasy_ffi::QuerySet for QuerySet {
    extern "C-unwind" fn get_local_column_index(&self, identity: &TypeIdentity) -> LocalColumnIndex {
        self.accessible_components
            .get(identity)
            .expect("this component isn't part of the query")
            .clone()
    }

    extern "C-unwind" fn columns_index_for_archetype(
        &self,
        archetype_index: ArchetypeIndex,
    ) -> FfiResult<FfiSlice<&ColumnIndex>, RegistryError> {
        self.archetypes
            .get(&archetype_index)
            .map(|v| v.as_slice().into())
            .ok_or(RegistryError::ArchetypeNotFound)
            .into()
    }
}
