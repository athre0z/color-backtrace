//! Colorful and clean backtraces on panic.
//!
//! This library aims to make panics a little less painful by nicely colorizing
//! them, skipping over frames of functions called after the panic was already
//! initiated and printing relevant source snippets. Frames of functions in your
//! application are colored in a different color (red) than those of
//! dependencies (green).
//!
//! ### Screenshot
//! ![Screenshot](https://i.imgur.com/bMnNdAj.png)
//!
//! ### Features
//! - Colorize backtraces to be easier on the eyes
//! - Show source snippets if source files are found on disk
//! - Print frames of application code vs dependencies in different color
//! - Hide all the frames after the panic was already initiated
//!
//! ### Installing the panic handler
//!
//! In your main function, just insert the following snippet. That's it!
//! ```rust
//! color_backtrace::install();
//! ```
//!
//! ### Controlling verbosity
//! The verbosity is configured via the `RUST_BACKTRACE` environment variable.
//! An unset `RUST_BACKTRACE` corresponds to [minimal](Verbosity::MINIMAL),
//! `RUST_BACKTRACE=1` to [medium](Verbosity::MEDIUM) and `RUST_BACKTRACE=full`
//! to [full](Verbosity::FULL) verbosity levels.

use backtrace;
use std::fs::File;
use std::io::BufReader;
use std::io::{BufRead, ErrorKind};
use std::panic::PanicInfo;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use term::{self, color, StderrTerminal};

// ============================================================================================== //
// [Result / Error types]                                                                         //
// ============================================================================================== //

type IOResult<T = ()> = Result<T, std::io::Error>;

// ============================================================================================== //
// [Verbosity management]                                                                         //
// ============================================================================================== //

/// Defines how verbose the backtrace is supposed to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verbosity {
    /// Print a small message including the panic payload and the panic location.
    MINIMAL,
    /// Everything in `MINIMAL` and additionally print a backtrace.
    MEDIUM,
    /// Everything in `MEDIUM` plus source snippets for all backtrace locations.
    FULL,
}

/// Query the verbosity level.
pub fn get_verbosity() -> Verbosity {
    match std::env::var("RUST_BACKTRACE") {
        Ok(ref x) if x == "full" => Verbosity::FULL,
        Ok(_) => Verbosity::MEDIUM,
        Err(_) => Verbosity::MINIMAL,
    }
}

// ============================================================================================== //
// [Panic handler and install logic]                                                              //
// ============================================================================================== //

/// Create a `color_backtrace` panic handler.
///
/// This can be used if you want to combine the handler with other handlers.
pub fn create_panic_handler() -> Box<dyn Fn(&PanicInfo<'_>) + 'static + Sync + Send> {
    let mutex = Mutex::new(());
    Box::new(move |pi| {
        // Prevent mixed up printing when multiple threads panic at once.
        let _lock = mutex.lock();

        PanicHandler::new(pi).go().unwrap();
    })
}

/// Install the `color_backtrace` handler, by calling `std::panic::set_hook`.
pub fn install() {
    std::panic::set_hook(create_panic_handler());
}

// ============================================================================================== //
// [Backtrace frame]                                                                              //
// ============================================================================================== //

struct Sym<'a, 'b> {
    handler: &'a mut PanicHandler<'b>,
    name: Option<String>,
    lineno: Option<u32>,
    filename: Option<PathBuf>,
}

impl<'a, 'b> Sym<'a, 'b> {
    /// Heuristically determine whether the symbol is likely to be part of a
    /// dependency. If it fails to detect some patterns in your code base, feel
    /// free to drop an issue / a pull request!
    fn is_dependency_code(&self) -> bool {
        static SYM_PREFIXES: &[&str] = &[
            "std::",
            "core::",
            "backtrace::backtrace::",
            "_rust_begin_unwind",
            "color_traceback::",
            "__rust_maybe_catch_panic",
            "__pthread",
            "_main",
            "main",
            "__scrt_common_main_seh",
            "BaseThreadInitThunk",
            "_start",
            "__libc_start_main",
            "start_thread",
        ];

        // Inspect name.
        if let Some(ref name) = self.name {
            if SYM_PREFIXES.iter().any(|x| name.starts_with(x)) {
                return true;
            }
        }

        static FILE_PREFIXES: &[&str] = &[
            "/rustc/",
            "src/libstd/",
            "src/libpanic_unwind/",
            "src/libtest/",
        ];

        // Inspect filename.
        if let Some(ref filename) = self.filename {
            let filename = filename.to_string_lossy();
            if FILE_PREFIXES.iter().any(|x| filename.starts_with(x))
                || filename.contains("/.cargo/registry/src/")
            {
                return true;
            }
        }

        false
    }

    fn print_source_if_avail(&mut self) -> IOResult {
        let (lineno, filename) = match (self.lineno, self.filename.as_ref()) {
            (Some(a), Some(b)) => (a, b),
            // Without a line number and file name, we can't sensibly proceed.
            _ => return Ok(()),
        };

        self.handler.print_source_if_avail(filename, lineno)
    }

    fn print_loc(&mut self, i: usize) -> IOResult {
        let is_dependency_code = self.is_dependency_code();
        let t = &mut self.handler.t;

        // Print frame index.
        write!(t, "{:>2}: ", i)?;

        // Print function name, if known.
        let name_fallback = "<unknown>".to_owned();
        let name = self.name.as_ref().unwrap_or(&name_fallback);
        t.fg(if is_dependency_code {
            color::GREEN
        } else {
            color::BRIGHT_RED
        })?;
        writeln!(t, "{}", name)?;
        t.reset()?;

        // Print source location, if known.
        if let Some(ref file) = self.filename {
            let filestr = file.to_str().unwrap_or("<bad utf8>");
            let lineno = self
                .lineno
                .map_or("<unknown line>".to_owned(), |x| x.to_string());
            writeln!(t, "    at {}:{}", filestr, lineno)?;
        } else {
            writeln!(t, "    at <unknown source file>")?;
        }

        // Maybe print source.
        if self.handler.v >= Verbosity::FULL {
            self.print_source_if_avail()?;
        }

        Ok(())
    }
}

// ============================================================================================== //
// [Core panic handler logic]                                                                     //
// ============================================================================================== //

struct PanicHandler<'a> {
    pi: &'a PanicInfo<'a>,
    v: Verbosity,
    t: Box<StderrTerminal>,
}

fn is_post_panic_code(name: &Option<String>) -> bool {
    static SYM_PREFIXES: &[&str] = &[
        "_rust_begin_unwind",
        "core::result::unwrap_failed",
        "core::panicking::panic_fmt",
        "color_backtrace::create_panic_handler",
        "std::panicking::begin_panic",
        "begin_panic_fmt",
    ];

    match name {
        Some(ref name) => SYM_PREFIXES.iter().any(|x| name.starts_with(x)),
        None => false,
    }
}

impl<'a> PanicHandler<'a> {
    fn print_source_if_avail(&mut self, filename: &Path, lineno: u32) -> IOResult {
        let file = match File::open(filename) {
            Ok(file) => file,
            Err(ref e) if e.kind() == ErrorKind::NotFound => return Ok(()),
            e @ Err(_) => e?,
        };

        // Extract relevant lines.
        let reader = BufReader::new(file);
        let start_line = lineno - 2.min(lineno - 1);
        let surrounding_src = reader.lines().skip(start_line as usize - 1).take(5);
        for (line, cur_line_no) in surrounding_src.zip(start_line..) {
            if cur_line_no == lineno {
                // Print actual source line with brighter color.
                self.t.fg(color::BRIGHT_WHITE)?;
                writeln!(self.t, "{:>8} > {}", cur_line_no, line?)?;
                self.t.reset()?;
            } else {
                writeln!(self.t, "{:>8} │ {}", cur_line_no, line?)?;
            }
        }

        Ok(())
    }

    fn print_backtrace(&mut self) -> IOResult {
        writeln!(self.t, "{:━^80}", " BACKTRACE ")?;

        // Collect frame info.
        let mut symbols = Vec::new();
        backtrace::trace(|x| {
            backtrace::resolve(x.ip(), |sym| {
                symbols.push((
                    sym.name().map(|x| x.to_string()),
                    sym.lineno(),
                    sym.filename().map(|x| x.into()),
                ));
            });

            true
        });

        // Try to find where the interesting part starts...
        let cutoff = symbols
            .iter()
            .rposition(|x| is_post_panic_code(&x.0))
            .map(|x| x + 1)
            .unwrap_or(0);

        if cutoff != 0 {
            let text = format!("({} post panic frames hidden)", cutoff);
            self.t.fg(color::BRIGHT_CYAN)?;
            writeln!(self.t, "{:^80}", text)?;
            self.t.reset()?;
        }

        // Turn them into `Symbol` objects and print them.
        let symbols = symbols.into_iter().skip(cutoff).zip(cutoff..);
        for ((name, lineno, filename), i) in symbols {
            let mut sym = Sym {
                handler: self,
                name,
                lineno,
                filename,
            };

            sym.print_loc(i)?;
        }

        Ok(())
    }

    fn print_panic_info(&mut self) -> IOResult {
        self.t.fg(color::RED)?;
        writeln!(self.t, "Oh noez! Panic! 💥")?;
        self.t.reset()?;

        // Print panic message.
        let payload = self
            .pi
            .payload()
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| self.pi.payload().downcast_ref::<&str>().map(|x| *x))
            .unwrap_or("<non string panic payload>");

        write!(self.t, "Message:  ")?;
        self.t.fg(color::CYAN)?;
        writeln!(self.t, "{}", payload)?;
        self.t.reset()?;

        // If known, print panic location.
        write!(self.t, "Location: ")?;
        if let Some(loc) = self.pi.location() {
            self.t.fg(color::MAGENTA)?;
            write!(self.t, "{}", loc.file())?;
            self.t.fg(color::WHITE)?;
            write!(self.t, ":")?;
            self.t.fg(color::MAGENTA)?;
            writeln!(self.t, "{}", loc.line())?;
            self.t.reset()?;
        } else {
            writeln!(self.t, "<unknown>")?;
        }

        // Print some info on how to increase verbosity.
        if self.v == Verbosity::MINIMAL {
            write!(self.t, "\nBacktrace omitted. Run with ")?;
            self.t.fg(color::BRIGHT_WHITE)?;
            write!(self.t, "RUST_BACKTRACE=1")?;
            self.t.reset()?;
            writeln!(self.t, " environment variable to display it.")?;
        }
        if self.v <= Verbosity::MEDIUM {
            if self.v == Verbosity::MEDIUM {
                // If exactly medium, no newline was printed before.
                writeln!(self.t)?;
            }

            write!(self.t, "Run with ")?;
            self.t.fg(color::BRIGHT_WHITE)?;
            write!(self.t, "RUST_BACKTRACE=full")?;
            self.t.reset()?;
            writeln!(self.t, " to include source snippets.")?;
        }

        Ok(())
    }

    fn go(mut self) -> IOResult {
        self.print_panic_info()?;

        if self.v >= Verbosity::MEDIUM {
            self.print_backtrace()?;
        }

        Ok(())
    }

    fn new(pi: &'a PanicInfo) -> Self {
        Self {
            pi,
            v: get_verbosity(),
            t: term::stderr().unwrap(),
        }
    }
}

// ============================================================================================== //
