use reflexion::erased::ErasedMutPointer;

#[test]
fn erased() {
    let mut value: i32 = 25;
    unsafe {
        let ptr = ErasedMutPointer::from_mut(&mut value);
        let cst_ref = ptr.as_erased_ref();
        assert_eq!(*cst_ref.cast::<i32>(), 25);
    }
}
