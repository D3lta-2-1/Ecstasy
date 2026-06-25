use reflexion::{
    erased::{Any, ErasedMutPointer},
    ffi_slice::FfiSlice,
};
use vtable::vtable;

#[test]
fn erased() {
    let mut value: i32 = 25;
    unsafe {
        let ptr = ErasedMutPointer::<Any>::from_mut(&mut value);
        let cst_ref = ptr.as_erased_ref();
        assert_eq!(*cst_ref.cast::<i32>(), 25);
    }
}

#[test]
fn slice_test() {
    let mut binding = [5, 3, 6, 54, 4];
    let slice = binding.as_mut();
    let ffi_slice: FfiSlice<&mut i32> = slice.into();
    let slice_got: &mut [i32] = ffi_slice.into();
    assert_eq!(slice_got, [5, 3, 6, 54, 4])
}

#[vtable]
trait MovingObject {
    extern "C-unwind" fn introduce_yourself(&self);
}

struct Vehicle {
    wheel_number: u32,
}

impl MovingObject for Vehicle {
    extern "C-unwind" fn introduce_yourself(&self) {
        println!("I'm a Vehicle and I got {} wheels", self.wheel_number);
    }
}

struct SomeKindOfLeggedThingy {
    leg_number: u32,
}

impl MovingObject for SomeKindOfLeggedThingy {
    extern "C-unwind" fn introduce_yourself(&self) {
        println!("I'm alive and I got {} legs", self.leg_number);
    }
}

#[test]
fn test_vtable() {
    let car = Vehicle { wheel_number: 4 };
    let cat = SomeKindOfLeggedThingy { leg_number: 4 };

    let car_handle = car.as_opaque();
    let cat_handle = cat.as_opaque();
    println!(
        "car function {:?}",
        (Vehicle::VTABLE.introduce_yourself)(car_handle)
    );
    println!(
        "cat function {:?}",
        (SomeKindOfLeggedThingy::VTABLE.introduce_yourself)(cat_handle)
    );
}
