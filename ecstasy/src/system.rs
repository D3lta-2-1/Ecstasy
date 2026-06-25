use crate::{
    event::{Consumer, Event, Producer},
    loader::{SchedulerBuilderLoader, SystemContextLoader},
    query::{Query, QueryBundle, QueryState},
};
use ecstasy_ffi::{
    self, BorrowedResource, ComponentMutability, EventIndex, EventUsage, SchedulerBuilderOpaque,
    System, SystemContextOpaque,
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
        let mut borrowed_resources = Vec::new();
        let state = F::Params::init_state(builder);
        F::Params::build_query_list(&state, &mut borrowed_resources);
        borrowed_resources.sort_unstable();

        // test is the system can be built
        for pair in borrowed_resources.array_windows::<2>() {
            match pair {
                [
                    BorrowedResource::Component {
                        component: component_a,
                        mutability: mutability_a,
                    },
                    BorrowedResource::Component {
                        component: component_b,
                        mutability: mutability_b,
                    },
                ] if component_a == component_b => {
                    assert_eq!(
                        *mutability_a,
                        ComponentMutability::Const,
                        "this system fetch the same component multiples times with conflicting mutabilities"
                    );
                    assert_eq!(
                        *mutability_b,
                        ComponentMutability::Const,
                        "this system fetch the same component multiples times with conflicting mutabilities"
                    );
                }
                [
                    BorrowedResource::Event { event: a, .. },
                    BorrowedResource::Event { event: b, .. },
                ] => assert_ne!(a, b, "the same event can't be used twice inside a system"),
                _ => (),
            }
        }

        FunctionSystem {
            system: self,
            state,
            borrowed_resources,
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

    /// gather all resources borrowed by this system to compute compatibility with other systems
    fn build_query_list(_state: &Self::State, _list: &mut Vec<BorrowedResource>);
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

    fn build_query_list(state: &Self::State, list: &mut Vec<BorrowedResource>) {
        list.extend_from_slice(state.get_borrowed_resources());
    }
}

impl<T: Event> SystemParam for Producer<'_, T> {
    type State = EventIndex;

    type Item<'scheduler, 'state> = Producer<'scheduler, T>;

    fn init_state(builder: &mut SchedulerBuilderOpaque) -> Self::State {
        SchedulerBuilderLoader::find_event(builder, T::DESCRIPTOR)
    }

    fn get_param<'scheduler, 'state>(
        state: &'state Self::State,
        ctx: &'scheduler SystemContextOpaque,
    ) -> Self::Item<'scheduler, 'state> {
        unsafe {
            let producer = SystemContextLoader::get_publisher(ctx, *state);
            Producer {
                inner: producer,
                phantom: std::marker::PhantomData,
            }
        }
    }

    fn build_query_list(state: &Self::State, list: &mut Vec<BorrowedResource>) {
        list.push(BorrowedResource::Event {
            usage: EventUsage::Producer,
            event: *state,
        });
    }
}

impl<T: Event> SystemParam for Consumer<'_, T> {
    type State = EventIndex;

    type Item<'scheduler, 'state> = Consumer<'scheduler, T>;

    fn init_state(builder: &mut SchedulerBuilderOpaque) -> Self::State {
        SchedulerBuilderLoader::find_event(builder, T::DESCRIPTOR)
    }

    fn get_param<'scheduler, 'state>(
        state: &'state Self::State,
        ctx: &'scheduler SystemContextOpaque,
    ) -> Self::Item<'scheduler, 'state> {
        unsafe {
            let consumer = SystemContextLoader::get_consumer(ctx, *state);
            Consumer {
                inner: consumer,
                phantom: std::marker::PhantomData,
            }
        }
    }

    fn build_query_list(state: &Self::State, list: &mut Vec<BorrowedResource>) {
        list.push(BorrowedResource::Event {
            usage: EventUsage::Consumer,
            event: *state,
        });
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

    fn build_query_list((state0,): &Self::State, list: &mut Vec<BorrowedResource>) {
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

    fn build_query_list((state0, state1): &Self::State, list: &mut Vec<BorrowedResource>) {
        T1::build_query_list(state0, list);
        T2::build_query_list(state1, list);
    }
}

/// Represent a system with its params
pub struct FunctionSystem<F: SystemParamFunction<Params> + 'static, Params: SystemParam> {
    system: F,
    state: <F::Params as SystemParam>::State,
    borrowed_resources: Vec<BorrowedResource>,
}

impl<F: SystemParamFunction<Params> + 'static, Params: SystemParam> System
    for FunctionSystem<F, Params>
{
    extern "C-unwind" fn call(&mut self, ctx: &SystemContextOpaque) {
        let arg = F::Params::get_param(&self.state, ctx);
        self.system.run(arg)
    }

    extern "C-unwind" fn borrowed_resources(&self) -> FfiSlice<&BorrowedResource> {
        self.borrowed_resources.as_slice().into()
    }
}
