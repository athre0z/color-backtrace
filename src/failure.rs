//! Temporary hack to allow printing of `failure::Backtrace` objects.

use crate::default_output_stream;

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

#[deprecated(
    since = "0.4",
    note = "Use `PanicPrinter::print_failure_trace` instead."
)]
pub unsafe fn print_backtrace(
    trace: &failure::Backtrace,
    printer: &crate::PanicPrinter,
) -> crate::IOResult {
    printer.print_failure_trace(trace, &mut default_output_stream())
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
            print_backtrace(e.backtrace(), &mut settings).unwrap();
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
            print_backtrace(e.backtrace(), &mut settings).unwrap();
        }
    }
}
