use std::{
    marker::PhantomData,
    ops::{Index, IndexMut},
    slice::GetDisjointMutError,
};

pub struct IndexStorage<K: From<usize> + Into<usize> + Copy, V> {
    storage: Vec<V>,
    phantom: PhantomData<K>,
}

impl<K: From<usize> + Into<usize> + Copy, V> Index<K> for IndexStorage<K, V> {
    type Output = V;

    fn index(&self, index: K) -> &Self::Output {
        &self.storage[index.into()]
    }
}

impl<K: From<usize> + Into<usize> + Copy, V> IndexMut<K> for IndexStorage<K, V> {
    fn index_mut(&mut self, index: K) -> &mut Self::Output {
        &mut self.storage[index.into()]
    }
}

impl<K: From<usize> + Into<usize> + Copy, V> Default for IndexStorage<K, V> {
    fn default() -> Self {
        Self {
            storage: Vec::new(),
            phantom: Default::default(),
        }
    }
}

impl<K: From<usize> + Into<usize> + Copy, V> IndexStorage<K, V> {
    pub fn push(&mut self, v: V) -> K {
        let new_index = self.storage.len();
        self.storage.push(v);
        K::from(new_index)
    }

    /// iterate trough all occupied keys
    pub fn keys(&self) -> impl Iterator<Item = K> {
        (0..self.storage.len()).map(|k| K::from(k))
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.storage.iter_mut()
    }

    /*pub fn emplace(&mut self, builder: impl FnOnce(K) -> V) -> K {
        let new_index = self.storage.len();
        let index = K::from(new_index);
        self.storage.push(builder(index));
        index
    }*/

    pub fn get_disjoint_mut<const N: usize>(
        &mut self,
        indices: [K; N],
    ) -> Result<[&mut V; N], GetDisjointMutError> {
        let indices = indices.map(|k| k.into());
        self.storage.get_disjoint_mut(indices)
    }
}
