use crate::{
    event::{Event, Publisher},
    loader::{SchedulerBuilderLoader, SystemContextLoader},
    query::{Query, QueryBundle, QueryState},
};
use ecstasy_ffi::{
    self, EventIndex, QuerySetIndex, SchedulerBuilderOpaque, System, SystemContextOpaque,
};
use reflexion::ffi_slice::FfiSlice;

/// Convert thing to system (to create a trait object)
pub trait IntoSystem<Params> {
    type System: System;

    fn into_system(self, builder: &mut SchedulerBuilderOpaque) -> Self::System;
}

/// Convert any function with only system params into a system
impl<F, Params: SystemParam> IntoSystem<Params> for F
where
    F: SystemParamFunction<Params>,
{
    type System = FunctionSystem<F, Params>;

    fn into_system(self, builder: &mut SchedulerBuilderOpaque) -> Self::System {
        let mut used_queries = Vec::new();
        let state = F::Params::init_state(builder);
        F::Params::build_query_list(&state, &mut used_queries);
        FunctionSystem {
            system: self,
            state,
            used_queries,
        }
    }
}

/// Function with only system params
pub trait SystemParamFunction<Marker>: 'static {
    type Params: SystemParam;
    fn run(&mut self, params: SystemParamItem<Self::Params>);
}

/// one param function
// TODO: ADD macro for tuple up to 16
impl<Func, P1: SystemParam> SystemParamFunction<(P1,)> for Func
where
    Func: Send + Sync + 'static,
    Func: FnMut(P1) + FnMut(SystemParamItem<P1>) -> () + 'static,
{
    type Params = (P1,);

    fn run(&mut self, params: SystemParamItem<Self::Params>) {
        let (p1,) = params;
        #[inline(always)]
        fn call_inner<P1>(mut f: impl FnMut(P1), p1: P1) {
            f(p1)
        }
        call_inner(self, p1);
    }
}

/// implemented by all objects that can be used as a parameter.
pub trait SystemParam {
    type State;
    type Item<'scheduler, 'state>: SystemParam<State = Self::State>;
    fn init_state(builder: &mut SchedulerBuilderOpaque) -> Self::State;
    fn get_param<'scheduler, 'state>(
        state: &'state Self::State,
        ctx: &'scheduler SystemContextOpaque,
    ) -> Self::Item<'scheduler, 'state>;

    fn build_query_list(_state: &Self::State, _list: &mut Vec<QuerySetIndex>) {}
}

pub type SystemParamItem<'r, 's, P> = <P as SystemParam>::Item<'r, 's>;

impl<Bundle: QueryBundle + 'static> SystemParam for Query<'_, '_, Bundle> {
    type State = QueryState<Bundle>;
    type Item<'scheduler, 'state> = Query<'scheduler, 'state, Bundle>;

    fn init_state(builder: &mut SchedulerBuilderOpaque) -> Self::State {
        QueryState::new(SchedulerBuilderLoader::registry(builder))
    }

    fn get_param<'scheduler, 'state>(
        state: &'state Self::State,
        ctx: &'scheduler SystemContextOpaque,
    ) -> Self::Item<'scheduler, 'state> {
        let registry = SystemContextLoader::registry(ctx);
        state.promote(registry)
    }

    fn build_query_list(state: &Self::State, list: &mut Vec<QuerySetIndex>) {
        list.push(state.id);
    }
}

impl<T: Event> SystemParam for Publisher<'_, T> {
    type State = EventIndex;

    type Item<'scheduler, 'state> = Publisher<'scheduler, T>;

    fn init_state(builder: &mut SchedulerBuilderOpaque) -> Self::State {
        SchedulerBuilderLoader::find_event(builder, T::DESCRIPTOR)
    }

    fn get_param<'scheduler, 'state>(
        _state: &'state Self::State,
        _ctx: &'scheduler SystemContextOpaque,
    ) -> Self::Item<'scheduler, 'state> {
        todo!()
    }
}

impl<T1> SystemParam for (T1,)
where
    T1: SystemParam,
{
    type State = (T1::State,);
    type Item<'r, 's> = (T1::Item<'r, 's>,);

    fn init_state(builder: &mut SchedulerBuilderOpaque) -> Self::State {
        (T1::init_state(builder),)
    }

    fn get_param<'r, 's>(
        (state0,): &'s Self::State,
        handle: &'r SystemContextOpaque,
    ) -> Self::Item<'r, 's> {
        (T1::get_param(state0, handle),)
    }

    fn build_query_list((state0,): &Self::State, list: &mut Vec<QuerySetIndex>) {
        T1::build_query_list(state0, list);
    }
}

impl<T1, T2> SystemParam for (T1, T2)
where
    T1: SystemParam,
    T2: SystemParam,
{
    type State = (T1::State, T2::State);
    type Item<'r, 's> = (T1::Item<'r, 's>, T2::Item<'r, 's>);

    fn init_state(builder: &mut SchedulerBuilderOpaque) -> Self::State {
        (T1::init_state(builder), T2::init_state(builder))
    }

    fn get_param<'r, 's>(
        (state0, state1): &'s Self::State,
        handle: &'r SystemContextOpaque,
    ) -> Self::Item<'r, 's> {
        (T1::get_param(state0, handle), T2::get_param(state1, handle))
    }

    fn build_query_list((state0, state1): &Self::State, list: &mut Vec<QuerySetIndex>) {
        T1::build_query_list(state0, list);
        T2::build_query_list(state1, list);
    }
}

/// Represent a system with its params
pub struct FunctionSystem<F: SystemParamFunction<Params> + 'static, Params: SystemParam> {
    system: F,
    state: <F::Params as SystemParam>::State,
    used_queries: Vec<QuerySetIndex>,
}

impl<F: SystemParamFunction<Params> + 'static, Params: SystemParam> System
    for FunctionSystem<F, Params>
{
    extern "C" fn call(&mut self, ctx: &SystemContextOpaque) {
        let arg = F::Params::get_param(&self.state, ctx);
        self.system.run(arg)
    }

    extern "C" fn query_list(&self) -> FfiSlice<&ecstasy_ffi::QuerySetIndex> {
        self.used_queries.as_slice().into()
    }
}
