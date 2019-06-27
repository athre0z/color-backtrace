//! Temporary hack to allow printing of `failure::Backtrace` objects.

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

/// Unsafely extract a reference to the internal `backtrace::Backtrace` from a
/// `failure::Backtrace`.
///
/// # Unsafe Usage
///
/// Casts a `failure::Backtrace` to an internal reimplementation of it's struct layout as of
/// failure `0.1.5` and then accesses its UnsafeCell to get a reference to its internal
/// Backtrace.
pub unsafe fn backdoortrace(_opaque: &failure::Backtrace) -> Option<&backtrace::Backtrace> {
    let _ = format!("{}", _opaque); // forces resolution
    let no_longer_opaque: &FakeBacktrace =
        { &*(_opaque as *const failure::Backtrace as *const FakeBacktrace) }; // unsafe
    if let Some(bt) = &no_longer_opaque.internal.backtrace {
        let bt = { &*bt.backtrace.get() }; // unsafe
        return Some(bt);
    }

    None
}

/// Extracts the internal `backtrace::Backtrace` from a `failure::Backtrace` and prints it, if one
/// exists. Prints that a backtrace was not capture if one is not found.
pub unsafe fn print_failure_backtrace(
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
        unsafe {
            print_failure_backtrace(e.backtrace(), &mut settings).unwrap();
        }
    }

    #[ignore]
    #[test]
    fn without_envvar() {
        if std::env::var("RUST_BACKTRACE").is_ok() {
            std::env::remove_var("RUST_BACKTRACE");
        }

        let e = failure::format_err!("arbitrary error :)");
        let mut settings = crate::Settings::default();
        unsafe {
            print_failure_backtrace(e.backtrace(), &mut settings).unwrap();
        }
    }
}
