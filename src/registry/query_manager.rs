use crate::registry::archetype_manager::ArchetypeManager;
use crate::registry::query::Query;
use crate::registry::{ArchetypeIndex, QueryIndex};
use crate::shared::id::Component;
use std::cmp::Ordering;
use std::collections::HashMap;

/// store and maintain Query.
/// Queries aren't deletable
#[derive(Default)]
pub struct QueryManager {
    queries: Vec<Query>, //I didn't find a smarter way than iterating through all queries to find candidats in case of an archetype match
    // since archetype creation should be occasional, it shouldn't be an issue
    builder_to_queries: HashMap<Vec<Component>, QueryIndex>,
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
    pub fn insert_query(
        &mut self,
        builder: Vec<Component>,
        builder_func: impl Fn(Vec<Component>) -> Query,
    ) -> QueryIndex {
        if let Some(index) = self.builder_to_queries.get(&builder) {
            return *index;
        }
        let index = self.queries.len();
        let query = builder_func(builder.clone());
        self.queries.push(query);
        self.builder_to_queries.insert(builder, index);
        index
    }

    pub fn get_query(&self, index: QueryIndex) -> &Query {
        &self.queries[index]
    }

    pub fn add_archetype(
        &mut self,
        archetype_index: ArchetypeIndex,
        archetypes: &ArchetypeManager,
    ) {
        for query in &mut self.queries {
            if contain(
                archetypes.get_archetype(archetype_index).get_descriptor(),
                query.requested_components(),
            ) {
                query.add_archetype(archetype_index, archetypes);
            }
        }
    }
}
