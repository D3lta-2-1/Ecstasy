use super::{ArchetypeIndex, QuerySetIndex, archetype_manager::ArchetypeManager, query::QuerySet};
use crate::index_storage::IndexStorage;
use ecstasy_ffi::{Component, QueryBuilder};
use std::{cmp::Ordering, collections::HashMap};

/// store and maintain Query.
/// Queries aren't deletable for now
#[derive(Default)]
pub struct QueryManager {
    query_sets: IndexStorage<QuerySetIndex, QuerySet>, //I didn't find a smarter way than iterating through all queries to find candidates in case of an archetype match
    // since archetype creation should be occasional, it shouldn't be an issue
    builder_to_query_sets: HashMap<Vec<Component>, QuerySetIndex>, //The whole point of querySet is optimizing maintenance cost (archetype addition)
}

fn contain<T: Ord>(container: &[T], contained: &[T]) -> bool {
    if container.len() < contained.len() {
        return false;
    }

    let mut container = container.iter();
    'outer: for a in contained {
        while let Some(b) = container.next() {
            match a.cmp(b) {
                Ordering::Less => return false,
                Ordering::Equal => continue 'outer,
                Ordering::Greater => (),
            }
        }
    }
    true
}

#[test]
fn test_contain() {
    assert!(contain(&[2, 3, 7, 9, 10], &[2, 7, 10]))
}

impl QueryManager {
    fn insert_query_set(
        &mut self,
        builder: Vec<Component>,
        queryset_builder: impl Fn(Vec<Component>) -> QuerySet,
    ) -> QuerySetIndex {
        if let Some(index) = self.builder_to_query_sets.get(&builder) {
            return *index;
        }
        let query_set = queryset_builder(builder.clone());
        let index = self.query_sets.push(query_set);
        self.builder_to_query_sets.insert(builder, index);
        index
    }

    pub fn get_query(
        &mut self,
        builder: QueryBuilder,
        queryset_builder: impl Fn(Vec<Component>) -> QuerySet,
    ) -> QuerySetIndex {
        let requested_components = builder.requested_components.to_vec();
        self.insert_query_set(requested_components, queryset_builder)
    }

    /// update all QuerySet that are concerned by the change
    pub fn add_archetype(
        &mut self,
        archetype_index: ArchetypeIndex,
        archetypes: &ArchetypeManager,
    ) {
        for query in self.query_sets.values_mut() {
            if contain(
                archetypes.get_archetype(archetype_index).get_descriptor(),
                query.requested_components(),
            ) {
                query.add_archetype(archetype_index, archetypes);
            }
        }
    }

    pub fn get_query_set(&self, id: QuerySetIndex) -> &QuerySet {
        &self.query_sets[id]
    }
}
