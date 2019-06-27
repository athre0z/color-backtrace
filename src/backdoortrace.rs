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

pub fn backdoortrace(_opaque: &failure::Backtrace) -> Option<&backtrace::Backtrace> {
    let _ = format!("{}", _opaque); // forces resolution
    let no_longer_opaque: &FakeBacktrace =
        unsafe { &*(_opaque as *const failure::Backtrace as *const FakeBacktrace) };
    if let Some(bt) = &no_longer_opaque.internal.backtrace {
        let bt = unsafe { &*bt.backtrace.get() };
        return Some(bt);
    }

    None
}

pub fn print_failure_backtrace(
    trace: &failure::Backtrace,
    settings: &mut crate::Settings,
) -> crate::IOResult {
    let internal = backdoortrace(trace);

    if let Some(internal) = internal {
        super::print_backtrace(internal, settings)
    } else {
        writeln!(settings.out, "<failure backtrace not captured>")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_envvar() {
        std::env::set_var("RUST_BACKTRACE", "full");

        let e = failure::format_err!("arbitrary error :)");
        let mut settings = crate::Settings::default();
        print_failure_backtrace(e.backtrace(), &mut settings).unwrap();
    }

    #[ignore]
    #[test]
    fn without_envvar() {
        if std::env::var("RUST_BACKTRACE").is_ok() {
            std::env::remove_var("RUST_BACKTRACE");
        }

        let e = failure::format_err!("arbitrary error :)");
        let mut settings = crate::Settings::default();
        print_failure_backtrace(e.backtrace(), &mut settings).unwrap();
    }
}
