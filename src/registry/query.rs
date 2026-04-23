use crate::registry::archetype_manager::ArchetypeManager;
use crate::registry::{ArchetypeIndex, ColumnIndex};
use crate::shared::id::{Component, ComponentIdentity};
use std::collections::HashMap;

pub type LocalColumnIndex = usize;
pub type QueryIndex = usize;

/// a Query is a shortcut to access archetypes, being part of the registry storage, that share common properties
/// a Query is able to quickly find all column that contain sized component
pub struct Query {
    pub(crate) requested_components: Vec<Component>, // contain all components, even unsized components
    pub(crate) accessible_components: HashMap<ComponentIdentity, LocalColumnIndex>, // only keep sized components,
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
            let colum = archetypes.find_column(component, archetype_index);
            mapping[*local_index] = colum;
        }
        self.archetypes.insert(archetype_index, mapping);
    }
}
