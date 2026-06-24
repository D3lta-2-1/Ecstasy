/// An Opaque type is used to represent pointers and references to a type which the size remain unknown at compilation
pub trait Opaque {
    type Handle<'a>;
    type Vtable;

    unsafe fn handle<'a>(handle: *mut Self, vtable: &'static Self::Vtable) -> Self::Handle<'a>;
}
