use std::{
    cmp::Ordering,
    collections::{BinaryHeap, VecDeque},
};

use ecstasy_ffi::{BorrowedResource, ComponentMutability, EventUsage};

use crate::scheduler::SystemEntry;

/// Comaptiblity implement Ord, when the largest element is "easiest constraint to satisfy" and the smallest is the most restrictive one
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Compatibility {
    Compatible,
    Incompatible,
    Earlier,
    Later,
}

impl PartialOrd for Compatibility {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Compatibility::Compatible, Compatibility::Compatible) => Some(Ordering::Equal),
            (Compatibility::Compatible, _) => Some(Ordering::Greater),
            (Compatibility::Incompatible, Compatibility::Incompatible) => Some(Ordering::Equal),
            (Compatibility::Incompatible, Compatibility::Earlier) => Some(Ordering::Greater),
            (Compatibility::Incompatible, Compatibility::Later) => Some(Ordering::Greater),
            (Compatibility::Earlier, Compatibility::Earlier) => Some(Ordering::Equal),
            (Compatibility::Earlier, Compatibility::Later) => None,
            (Compatibility::Later, Compatibility::Later) => Some(Ordering::Equal),

            (a, b) => b.partial_cmp(a).map(Ordering::reverse),
        }
    }
}

impl Compatibility {
    /// Compute if a pair of resources can be executed at the same time
    fn between_resource(a: &BorrowedResource, b: &BorrowedResource) -> Self {
        match (a, b) {
            (
                BorrowedResource::Event {
                    event: event_a,
                    usage: usage_a,
                },
                BorrowedResource::Event {
                    event: event_b,
                    usage: usage_b,
                },
            ) if event_a == event_b => match (usage_a, usage_b) {
                (EventUsage::Producer, EventUsage::Producer) => Self::Incompatible,
                (EventUsage::Producer, EventUsage::Consumer) => Self::Earlier,
                (EventUsage::Consumer, EventUsage::Producer) => Self::Later,
                (EventUsage::Consumer, EventUsage::Consumer) => Self::Compatible,
            },
            (
                BorrowedResource::Component {
                    component: component_a,
                    mutability: mutability_a,
                },
                BorrowedResource::Component {
                    component: component_b,
                    mutability: mutability_b,
                },
            ) if component_a == component_b => match (mutability_a, mutability_b) {
                (ComponentMutability::Const, ComponentMutability::Const) => {
                    Compatibility::Compatible
                }
                (ComponentMutability::Mut, _) => Compatibility::Incompatible,
                (_, ComponentMutability::Mut) => Compatibility::Incompatible,
            },

            _ => Compatibility::Compatible,
        }
    }

    /// try to make downgrade compatibility, panic if it needs to provide more rights, or the operation is simply undefined.
    fn restrain_to(&mut self, new_value: Self) {
        assert_eq!(
            new_value.partial_cmp(self),
            Some(Ordering::Less),
            "unable to restrain {:?} to {:?}",
            self,
            new_value
        );
    }

    /// compute the compatibility between to list of Borrowed Resources
    fn between_resources(left: &[BorrowedResource], right: &[BorrowedResource]) -> Self {
        let mut compatibility = Self::Compatible;

        let mut left_iter = left.iter().cloned().peekable();
        let mut right_iter = right.iter().cloned().peekable();

        loop {
            match (left_iter.peek(), right_iter.peek()) {
                (Some(l), Some(r)) => {
                    compatibility.restrain_to(Self::between_resource(l, r));
                    match l.cmp(r) {
                        Ordering::Greater => {
                            let _ = right_iter.next();
                        }
                        Ordering::Less => {
                            let _ = left_iter.next();
                        }
                        Ordering::Equal => {
                            let _ = right_iter.next();
                            let _ = left_iter.next();
                        }
                    }
                }
                _ => break,
            };
        }
        compatibility
    }
}

type SystemVertex = usize;

struct ExecutedLater(SystemVertex);

/// Store a graph in which each vertex is a System, and edges represent ordering constraint between them
pub struct CompatibilityGraph {
    /// vertex indices are in 0..system_count
    system_count: usize,
    /// store the ranges of neighbor's indices for the N-th vertex
    neighbor_indices: Vec<usize>,
    later_constraints: Vec<ExecutedLater>,
}

impl CompatibilityGraph {
    pub fn new(systems: &[SystemEntry]) -> Self {
        let system_count = systems.len();
        let mut neighbor_indices = Vec::with_capacity(system_count + 1);
        neighbor_indices.push(0);
        let mut later_constraints = Vec::new();

        #[derive(PartialEq, Eq)]
        struct QueueItem {
            src: SystemVertex,
            dst: SystemVertex,
        }

        // the smallest src then dst in first
        impl Ord for QueueItem {
            fn cmp(&self, other: &Self) -> Ordering {
                other.src.cmp(&self.src).then(other.dst.cmp(&self.dst))
            }
        }

        impl PartialOrd for QueueItem {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        let mut neighbor = 0;
        let mut queue = BinaryHeap::new();

        for (i, system_a) in systems.iter().enumerate() {
            while queue.peek().is_some_and(|v: &QueueItem| v.src == i) {
                later_constraints.push(ExecutedLater(queue.pop().unwrap().dst));
                neighbor += 1;
            }

            for (j, system_b) in systems.iter().enumerate().skip(i + 1) {
                let comp = Compatibility::between_resources(
                    system_a.borrowed_resources(),
                    system_b.borrowed_resources(),
                );
                match comp {
                    Compatibility::Later => {
                        later_constraints.push(ExecutedLater(j));
                        neighbor += 1;
                    }
                    Compatibility::Earlier => queue.push(QueueItem { src: j, dst: i }),
                    _ => (),
                }
            }
            neighbor_indices.push(neighbor);
        }

        assert!(queue.is_empty());
        assert_eq!(neighbor_indices.len() - 1, systems.len());

        Self {
            system_count,
            neighbor_indices,
            later_constraints,
        }
    }

    fn get_neighbors(&self, vertex: SystemVertex) -> &[ExecutedLater] {
        &self.later_constraints[self.neighbor_indices[vertex]..self.neighbor_indices[vertex + 1]]
    }

    /// return an order in which all system are executed given their Ordering preferences
    pub fn topological_sort(&self) -> Vec<usize> {
        let mut vertex_states = vec![VertexState::Untouched; self.system_count];
        let mut order = Vec::with_capacity(self.system_count);

        #[derive(Clone, Copy)]
        enum VertexState {
            Untouched,
            InProcess,
            AlreadyProcessed,
        }

        fn explorate(
            graph: &CompatibilityGraph,
            vertex: SystemVertex,
            vertex_states: &mut [VertexState],
            order: &mut Vec<usize>,
        ) {
            vertex_states[vertex] = VertexState::InProcess;
            for &ExecutedLater(neighbor) in graph.get_neighbors(vertex) {
                match vertex_states[neighbor] {
                    VertexState::Untouched => explorate(graph, neighbor, vertex_states, order),
                    VertexState::InProcess => panic!(
                        "conflicting system detected, impossible to resolve an executing order"
                    ), // loop detection.
                    VertexState::AlreadyProcessed => continue,
                }
            }
            vertex_states[vertex] = VertexState::AlreadyProcessed;
            order.push(vertex);
        }

        for i in 0..self.system_count {
            match vertex_states[i] {
                VertexState::Untouched => explorate(self, i, &mut vertex_states, &mut order),
                VertexState::InProcess => unreachable!(),
                VertexState::AlreadyProcessed => continue,
            }
        }

        order.reverse();
        assert_eq!(order.len(), self.system_count);
        order
    }
}
