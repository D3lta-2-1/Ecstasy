use crate::{ArchetypeIndex, ColumnIndex, archetype_manager::ArchetypeManager};
use reflexion::{ffi_enum::FfiResult, ffi_slice::FfiSlice};
use registry_ffi::{Component, ComponentIdentity, LocalColumnIndex, RegistryError};
use std::collections::HashMap;

/// a Query is a shortcut to access archetypes, being part of the registry storage, that share common properties
/// a Query is able to quickly find all column that contain sized component
/// it gather all archetype that share a common set of components, but doesn't care about mutability
pub struct Query {
    pub(crate) requested_components: Vec<Component>, // contain all components, even unsized components
    pub(crate) accessible_components: HashMap<ComponentIdentity, LocalColumnIndex>, // only keep sized components, should I store the "associated components instead ?"
    pub(crate) archetypes: HashMap<ArchetypeIndex, Vec<ColumnIndex>>, //where stuff is located in the archetype.
}

impl Query {
    pub fn requested_components(&self) -> &[Component] {
        &self.requested_components
    }

    pub fn add_archetype(
        &mut self,
        archetype_index: ArchetypeIndex,
        archetypes: &ArchetypeManager,
    ) {
        let mut mapping = vec![0; self.accessible_components.len()];
        for (component, local_index) in self.accessible_components.iter() {
            let component = archetypes
                .find_component(component)
                .expect("this archetype don't match the query requirements");
            let column = archetypes.find_column(component, archetype_index);
            mapping[*local_index] = column;
        }
        self.archetypes.insert(archetype_index, mapping);
    }
}

impl registry_ffi::Query for Query {
    extern "C" fn get_local_column_index(&self, identity: &ComponentIdentity) -> LocalColumnIndex {
        self.accessible_components
            .get(identity)
            .expect("this component isn't part of the query")
            .clone()
    }

    extern "C" fn columns_index_for_archetype(
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
