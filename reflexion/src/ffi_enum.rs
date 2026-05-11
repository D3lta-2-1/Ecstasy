/// An FFI wrappers for result.
#[repr(C)]
pub enum FfiResult<T, E> {
    Ok(T),
    Err(E),
}

impl<T, E> From<Result<T, E>> for FfiResult<T, E> {
    fn from(value: Result<T, E>) -> Self {
        match value {
            Ok(v) => FfiResult::Ok(v),
            Err(e) => FfiResult::Err(e),
        }
    }
}

impl<T, E> From<FfiResult<T, E>> for Result<T, E> {
    fn from(value: FfiResult<T, E>) -> Self {
        match value {
            FfiResult::Ok(v) => Ok(v),
            FfiResult::Err(e) => Err(e),
        }
    }
}

/// if ``FfiOption`` can be used everywhere safely, but it disable Null pointer optimisation
/// when the contained object can use the null pointer optimisation, it's worth using std::Option instead
#[repr(C)]
pub enum FfiOption<T> {
    Some(T),
    None,
}

impl<T> From<Option<T>> for FfiOption<T> {
    fn from(value: Option<T>) -> Self {
        match value {
            Some(v) => FfiOption::Some(v),
            None => FfiOption::None,
        }
    }
}

impl<T> From<FfiOption<T>> for Option<T> {
    fn from(value: FfiOption<T>) -> Self {
        match value {
            FfiOption::Some(v) => Some(v),
            FfiOption::None => None,
        }
    }
}
