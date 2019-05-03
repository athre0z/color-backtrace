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
//! An unset `RUST_BACKTRACE` corresponds to [minimal](Verbosity::Minimal),
//! `RUST_BACKTRACE=1` to [medium](Verbosity::Medium) and `RUST_BACKTRACE=full`
//! to [full](Verbosity::Full) verbosity levels.

use backtrace;
use std::fs::File;
use std::io::{BufRead, BufReader, ErrorKind, Write};
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
    Minimal,
    /// Everything in `Minimal` and additionally print a backtrace.
    Medium,
    /// Everything in `Medium` plus source snippets for all backtrace locations.
    Full,
}

impl Verbosity {
    /// Query the verbosity level.
    pub fn get() -> Self {
        match std::env::var("RUST_BACKTRACE") {
            Ok(ref x) if x == "full" => Verbosity::Full,
            Ok(_) => Verbosity::Medium,
            Err(_) => Verbosity::Minimal,
        }
    }
}
// ============================================================================================== //
// [Panic handler and install logic]                                                              //
// ============================================================================================== //

/// Create a `color_backtrace` panic handler.
///
/// This can be used if you want to combine the handler with other handlers.
pub fn create_panic_handler(
    settings: Settings,
) -> Box<dyn Fn(&PanicInfo<'_>) + 'static + Sync + Send> {
    // Aside from syncing access to the settings, this also prevents mixed up
    // printing when multiple threads panic at once.
    let settings_mutex = Mutex::new(settings);
    Box::new(move |pi| {
        let mut settings_lock = settings_mutex.lock().unwrap();
        PanicHandler::new(pi, &mut *settings_lock).go().unwrap();
    })
}

/// Install the `color_backtrace` handler with default settings.
pub fn install() {
    std::panic::set_hook(create_panic_handler(Settings::new()))
}

/// Install the `color_backtrace` handler with custom settings.
pub fn install_with_settings(settings: Settings) {
    std::panic::set_hook(create_panic_handler(settings))
}

// ============================================================================================== //
// [Backtrace frame]                                                                              //
// ============================================================================================== //

struct Frame<'a, 'b> {
    handler: &'a mut PanicHandler<'b>,
    name: Option<String>,
    lineno: Option<u32>,
    filename: Option<PathBuf>,
}

impl<'a, 'b> Frame<'a, 'b> {
    /// Heuristically determine whether the symbol is likely to be part of a
    /// dependency. If it fails to detect some patterns in your code base, feel
    /// free to drop an issue / a pull request!
    fn is_dependency_code(&self) -> bool {
        const SYM_PREFIXES: &[&str] = &[
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

        const FILE_PREFIXES: &[&str] = &[
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
        let t = &mut self.handler.s.out;

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
        if self.handler.v >= Verbosity::Full {
            self.print_source_if_avail()?;
        }

        Ok(())
    }
}

// ============================================================================================== //
// [Settings]                                                                                     //
// ============================================================================================== //

// #[derive(Debug, Clone)]
pub struct Settings {
    message: String,
    out: Box<dyn PanicOutputStream>,
}

impl Settings {
    pub fn new() -> Self {
        Self {
            message: "The application panicked (crashed).".to_owned(),
            out: Box::new(ColorizedStderrOutput::new(term::stderr().unwrap())),
        }
    }

    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    pub fn output_stream(mut self, out: Box<dyn PanicOutputStream>) -> Self {
        self.out = out;
        self
    }
}

// ============================================================================================== //
// [Term output abtraction]                                                                       //
// ============================================================================================== //

pub trait Colorize {
    fn fg(&mut self, color: color::Color) -> IOResult;
    fn bg(&mut self, color: color::Color) -> IOResult;
    fn reset(&mut self) -> IOResult;
}

pub trait PanicOutputStream: Colorize + Write + Send {}

// ---------------------------------------------------------------------------------------------- //
// [ColorizedStderrOutput]                                                                        //
// ---------------------------------------------------------------------------------------------- //

/// Output implementation that prints to `stderr`, with colors enabled.
pub struct ColorizedStderrOutput {
    term: Box<StderrTerminal>,
}

impl ColorizedStderrOutput {
    pub fn new(term: Box<StderrTerminal>) -> Self {
        Self { term }
    }
}

impl Colorize for ColorizedStderrOutput {
    fn fg(&mut self, color: color::Color) -> IOResult {
        Ok(self.term.fg(color)?)
    }

    fn bg(&mut self, color: color::Color) -> IOResult {
        Ok(self.term.bg(color)?)
    }

    fn reset(&mut self) -> IOResult {
        Ok(self.term.reset()?)
    }
}

impl Write for ColorizedStderrOutput {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.term.get_mut().write(buf)
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.term.get_mut().flush()
    }
}

impl PanicOutputStream for ColorizedStderrOutput {}

// ---------------------------------------------------------------------------------------------- //
// [StreamOutput]                                                                                 //
// ---------------------------------------------------------------------------------------------- //

/// Output implementation printing to an arbitraty `std::io::Write` stream,
/// without colors.
pub struct StreamOutput<T: Write + Send> {
    stream: T,
}

impl<T: Write + Send> StreamOutput<T> {
    pub fn new(stream: T) -> Self {
        Self { stream }
    }
}

impl<T: Write + Send> Write for StreamOutput<T> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.stream.write(buf)
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.stream.flush()
    }
}

impl<T: Write + Send> Colorize for StreamOutput<T> {
    fn fg(&mut self, _color: color::Color) -> IOResult {
        Ok(())
    }

    fn bg(&mut self, _color: color::Color) -> IOResult {
        Ok(())
    }

    fn reset(&mut self) -> IOResult {
        Ok(())
    }
}

impl<T: Write + Send> PanicOutputStream for StreamOutput<T> {}

// ============================================================================================== //
// [Core panic handler logic]                                                                     //
// ============================================================================================== //

struct PanicHandler<'a> {
    pi: &'a PanicInfo<'a>,
    v: Verbosity,
    s: &'a mut Settings,
}

fn is_post_panic_code(name: &Option<String>) -> bool {
    const SYM_PREFIXES: &[&str] = &[
        "_rust_begin_unwind",
        "core::result::unwrap_failed",
        "core::panicking::panic_fmt",
        "color_backtrace::create_panic_handler",
        "std::panicking::begin_panic",
        "begin_panic_fmt",
    ];

    match name {
        Some(name) => SYM_PREFIXES.iter().any(|x| name.starts_with(x)),
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
                self.s.out.fg(color::BRIGHT_WHITE)?;
                writeln!(self.s.out, "{:>8} > {}", cur_line_no, line?)?;
                self.s.out.reset()?;
            } else {
                writeln!(self.s.out, "{:>8} │ {}", cur_line_no, line?)?;
            }
        }

        Ok(())
    }

    fn print_backtrace(&mut self) -> IOResult {
        writeln!(self.s.out, "{:━^80}", " BACKTRACE ")?;

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
            self.s.out.fg(color::BRIGHT_CYAN)?;
            writeln!(self.s.out, "{:^80}", text)?;
            self.s.out.reset()?;
        }

        // Turn them into `Frame` objects and print them.
        let symbols = symbols.into_iter().skip(cutoff).zip(cutoff..);
        for ((name, lineno, filename), i) in symbols {
            let mut sym = Frame {
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
        self.s.out.fg(color::RED)?;
        writeln!(self.s.out, "{}", self.s.message)?;
        self.s.out.reset()?;

        // Print panic message.
        let payload = self
            .pi
            .payload()
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| self.pi.payload().downcast_ref::<&str>().map(|x| *x))
            .unwrap_or("<non string panic payload>");

        write!(self.s.out, "Message:  ")?;
        self.s.out.fg(color::CYAN)?;
        writeln!(self.s.out, "{}", payload)?;
        self.s.out.reset()?;

        // If known, print panic location.
        write!(self.s.out, "Location: ")?;
        if let Some(loc) = self.pi.location() {
            self.s.out.fg(color::MAGENTA)?;
            write!(self.s.out, "{}", loc.file())?;
            self.s.out.fg(color::WHITE)?;
            write!(self.s.out, ":")?;
            self.s.out.fg(color::MAGENTA)?;
            writeln!(self.s.out, "{}", loc.line())?;
            self.s.out.reset()?;
        } else {
            writeln!(self.s.out, "<unknown>")?;
        }

        // Print some info on how to increase verbosity.
        if self.v == Verbosity::Minimal {
            write!(self.s.out, "\nBacktrace omitted. Run with ")?;
            self.s.out.fg(color::BRIGHT_WHITE)?;
            write!(self.s.out, "RUST_BACKTRACE=1")?;
            self.s.out.reset()?;
            writeln!(self.s.out, " environment variable to display it.")?;
        }
        if self.v <= Verbosity::Medium {
            if self.v == Verbosity::Medium {
                // If exactly medium, no newline was printed before.
                writeln!(self.s.out)?;
            }

            write!(self.s.out, "Run with ")?;
            self.s.out.fg(color::BRIGHT_WHITE)?;
            write!(self.s.out, "RUST_BACKTRACE=full")?;
            self.s.out.reset()?;
            writeln!(self.s.out, " to include source snippets.")?;
        }

        Ok(())
    }

    fn go(mut self) -> IOResult {
        self.print_panic_info()?;

        if self.v >= Verbosity::Medium {
            self.print_backtrace()?;
        }

        Ok(())
    }

    fn new(pi: &'a PanicInfo, settings: &'a mut Settings) -> Self {
        Self {
            pi,
            s: settings,
            v: Verbosity::get(),
        }
    }
}

// ============================================================================================== //
