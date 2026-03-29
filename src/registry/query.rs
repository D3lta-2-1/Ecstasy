use crate::registry::archetype::Archetype;
use crate::registry::archetype_manager::ArchetypeManager;
use crate::registry::{ArchetypeIndex, ColumnIndex};
use crate::shared::id::Component;
use std::collections::HashMap;

type LocalColumnIndex = usize;

pub struct QueryBuilder {
    pub requested_components: Vec<Component>,
}

pub struct Query {
    requested_components: Vec<Component>, // contain all components, even unsized components
    component_map: HashMap<Component, LocalColumnIndex>, // only keep sized components,
    archetypes: HashMap<ArchetypeIndex, Vec<ColumnIndex>>, //where stuff is located in the archetype.
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
        let mut mapping = vec![0; self.component_map.len()];
        for (component, local_index) in self.component_map.iter() {
            let colum = archetypes.find_column(*component, archetype_index);
            mapping[*local_index] = colum;
        }
        self.archetypes.insert(archetype_index, mapping);
    }
}
