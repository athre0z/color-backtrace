struct FakeBacktrace {
    internal: FakeInternalBacktrace,
}

struct FakeInternalBacktrace {
    backtrace: Option<MaybeResolved>,
}

struct MaybeResolved {
    _resolved: std::sync::Mutex<bool>,
    backtrace: std::cell::UnsafeCell<backtrace::Backtrace>,
}

pub fn copy_internal_backtrace(_opaque: &failure::Backtrace) -> Option<backtrace::Backtrace> {
    let _ = format!("{}", _opaque); // forces resolution
    let no_longer_opaque: &FakeBacktrace =
        unsafe { &*(_opaque as *const failure::Backtrace as *const FakeBacktrace) };
    if let Some(bt) = &no_longer_opaque.internal.backtrace {
        let bt = unsafe { &*bt.backtrace.get() };
        return Some(bt.clone());
    }

    None
}

pub fn print_backdoortrace(
    trace: &failure::Backtrace,
    settings: &mut crate::Settings,
) -> crate::IOResult {
    let internal = copy_internal_backtrace(trace);

    if let Some(internal) = internal {
        super::print_backtrace(internal, settings)
    } else {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let e = failure::format_err!("arbitrary error :)");
        let mut settings = crate::Settings::default();
        print_backdoortrace(e.backtrace(), &mut settings).unwrap();
    }
}
