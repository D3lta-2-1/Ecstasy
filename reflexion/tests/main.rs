use reflexion::erased::ErasedMutPointer;
use reflexion::ffi_slice::FiiSlice;

#[test]
fn erased() {
    let mut value: i32 = 25;
    unsafe {
        let ptr = ErasedMutPointer::from_mut(&mut value);
        let cst_ref = ptr.as_erased_ref();
        assert_eq!(*cst_ref.cast::<i32>(), 25);
    }
}

#[test]
fn slice_test() {
    let mut binding = [5,3,6,54,4];
    let slice = binding.as_mut();
    let ffi_slice: FiiSlice<&mut i32> = slice.into();
    let slice_got: &mut [i32] = ffi_slice.into();
    assert_eq!(slice_got, [5,3,6,54,4])
}