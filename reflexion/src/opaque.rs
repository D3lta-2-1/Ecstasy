/// An Opaque type is used to represent pointers and references to a type which the size remain unknown at compilation
pub trait Opaque {
    type Handle<'a>;
    type MutHandle<'a>;
    type Vtable;

    unsafe fn handle<'a>(handle: *const Self, vtable: &'static Self::Vtable) -> Self::Handle<'a>;
    unsafe fn mut_handle<'a>(
        handle: *mut Self,
        vtable: &'static Self::Vtable,
    ) -> Self::MutHandle<'a>;
}
