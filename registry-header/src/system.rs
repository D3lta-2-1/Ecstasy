use crate::query::{Query, QueryBundle, QueryState};
use reflexion::ffi_slice::FfiSlice;
use registry_ffi::{RegistryHandle, RegistryMutHandle, System};

/// Convert thing to system (to create a trait object)
pub trait IntoSystem<Params> {
    type System: System;

    fn into_system(self, handle: &mut RegistryMutHandle) -> Self::System;
}

/// Convert any function with only system params into a system
impl<F, Params: SystemParam> IntoSystem<Params> for F
where
    F: SystemParamFunction<Params>,
{
    type System = FunctionSystem<F, Params>;

    fn into_system(self, handle: &mut RegistryMutHandle) -> Self::System {
        FunctionSystem {
            system: self,
            cache: F::Params::init_state(handle),
        }
    }
}

/// Function with only system params
pub trait SystemParamFunction<Marker>: 'static {
    type Params: SystemParam;
    fn run(&mut self, parmas: SystemParamItem<Self::Params>);
}

/// one param function
impl<Func, P1: SystemParam> SystemParamFunction<(P1,)> for Func
where
    Func: Send + Sync + 'static,
    Func: FnMut(P1) + FnMut(SystemParamItem<P1>) -> () + 'static,
{
    type Params = (P1,);

    fn run(&mut self, param: SystemParamItem<Self::Params>) {
        let (p1,) = param;
        fn call_inner<P1>(mut f: impl FnMut(P1), p1: P1) {
            f(p1)
        }
        call_inner(self, p1);
    }
}

/*impl<F, P1: SystemParam, P2: SystemParam> SystemParamFunction<(P1, P2)> for F
where
    F: FnMut(P1, P2) -> () + 'static,
{
    fn run(&mut self, (parms1, parms2): (P1, P2)) {
        self(parms1, parms2)
    }
}*/

/// implemented by all objects that can be used as a parameter.
pub trait SystemParam {
    type State;
    type Item<'registry, 'state>: SystemParam<State = Self::State>;
    fn init_state(handle: &mut RegistryMutHandle) -> Self::State;
    fn get_param<'registry, 'state>(
        state: &'state Self::State,
        handle: RegistryHandle<'registry>,
    ) -> Self::Item<'registry, 'state>;
}

pub type SystemParamItem<'r, 's, P> = <P as SystemParam>::Item<'r, 's>;

impl<Bundle: QueryBundle + 'static> SystemParam for Query<'_, '_, Bundle> {
    type State = QueryState<Bundle>;
    type Item<'registry, 'state> = Query<'registry, 'state, Bundle>;

    fn init_state(handle: &mut RegistryMutHandle) -> Self::State {
        QueryState::new(handle)
    }

    fn get_param<'registry, 'state>(
        state: &'state Self::State,
        handle: RegistryHandle<'registry>,
    ) -> Self::Item<'registry, 'state> {
        state.promote(handle)
    }
}

impl<T1> SystemParam for (T1,)
where
    T1: SystemParam,
{
    type State = (T1::State,);
    type Item<'r, 's> = (T1::Item<'r, 's>,);

    fn init_state(handle: &mut RegistryMutHandle) -> Self::State {
        (T1::init_state(handle),)
    }

    fn get_param<'r, 's>(
        (state0,): &'s Self::State,
        handle: RegistryHandle<'r>,
    ) -> Self::Item<'r, 's> {
        (T1::get_param(state0, handle),)
    }
}

impl<T1, T2> SystemParam for (T1, T2)
where
    T1: SystemParam,
    T2: SystemParam,
{
    type State = (T1::State, T2::State);
    type Item<'r, 's> = (T1::Item<'r, 's>, T2::Item<'r, 's>);

    fn init_state(handle: &mut RegistryMutHandle) -> Self::State {
        (T1::init_state(handle), T2::init_state(handle))
    }

    fn get_param<'r, 's>(
        (state0, state1): &'s Self::State,
        handle: RegistryHandle<'r>,
    ) -> Self::Item<'r, 's> {
        (T1::get_param(state0, handle), T2::get_param(state1, handle))
    }
}

/// Represent a system with its params
pub struct FunctionSystem<F: SystemParamFunction<Params> + 'static, Params: SystemParam> {
    system: F,
    cache: <F::Params as SystemParam>::State,
}

impl<F: SystemParamFunction<Params> + 'static, Params: SystemParam> System
    for FunctionSystem<F, Params>
{
    extern "C" fn call(&mut self, handle: RegistryHandle) {
        let arg = F::Params::get_param(&self.cache, handle);
        self.system.run(arg)
    }

    extern "C" fn query_list(&self) -> FfiSlice<&registry_ffi::Query> {
        todo!()
    }
}
