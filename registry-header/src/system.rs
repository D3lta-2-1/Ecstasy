use std::marker::PhantomData;

use reflexion::ffi_slice::FfiSlice;
use registry_ffi::{RegistryHandle, RegistryMutHandle, System};

use crate::query::{Query, QueryBundle, QueryHeaderData};

/*pub trait SystemProvider {
    fn as_system(self, handle: &mut RegistryMutHandle) -> impl System;
}

impl<QUERY: QueryBundle, T: FnMut(Query<'_, QUERY>)> SystemProvider for (T, PhantomData<QUERY>) {
    fn as_system(self, handle: &mut RegistryMutHandle) -> impl System {
        let header = QueryHeaderData::<QUERY>::new(handle);
        let query_list = [header.id];

        Executor1 {
            executor: self.0,
            header,
            query_list,
        }
    }
}*/

struct Executor1<T, QUERY: QueryBundle>
where
    T: FnMut(Query<'_, QUERY>),
{
    executor: T,
    header: QueryHeaderData<QUERY>,
    query_list: [registry_ffi::Query; 1],
}

impl<T, QUERY: QueryBundle> System for Executor1<T, QUERY>
where
    T: FnMut(Query<'_, QUERY>),
{
    extern "C" fn call(&mut self, registry: RegistryHandle) {
        (self.executor)(self.header.promote(registry))
    }

    extern "C" fn query_list(&self) -> FfiSlice<&registry_ffi::Query> {
        self.query_list.as_slice().into()
    }
}
