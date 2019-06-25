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
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::panic::PanicInfo;
use std::path::PathBuf;
use std::sync::Mutex;
use term::{self, color, Attr, StderrTerminal};

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
    /// Get the verbosity level from the `RUST_BACKTRACE` env variable.
    pub fn from_env() -> Self {
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
        if let Err(e) = print_panic_info(pi, &mut *settings_lock) {
            // Panicing while handling a panic would send us into a deadlock,
            // so we just print the error to stderr instead.
            eprintln!("Error while printing panic: {:?}", e);
        }
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

struct Frame {
    name: Option<String>,
    lineno: Option<u32>,
    filename: Option<PathBuf>,
}

impl Frame {
    /// Heuristically determine whether the frame is likely to be part of a
    /// dependency.
    ///
    /// If it fails to detect some patterns in your code base, feel free to drop
    /// an issue / a pull request!
    fn is_dependency_code(&self) -> bool {
        const SYM_PREFIXES: &[&str] = &[
            "std::",
            "core::",
            "backtrace::backtrace::",
            "_rust_begin_unwind",
            "color_traceback::",
            "__rust_",
            "___rust_",
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

    // Heuristically determine whether a frame is likely to be a post panic
    // frame.
    //
    // Post panic frames are frames of a functions called after the actual panic
    // is already in progress and don't contain any useful information for a
    // reader of the backtrace.
    fn is_post_panic_code(&self) -> bool {
        const SYM_PREFIXES: &[&str] = &[
            "_rust_begin_unwind",
            "core::result::unwrap_failed",
            "core::panicking::panic_fmt",
            "color_backtrace::create_panic_handler",
            "std::panicking::begin_panic",
            "begin_panic_fmt",
        ];

        match self.name.as_ref() {
            Some(name) => SYM_PREFIXES.iter().any(|x| name.starts_with(x)),
            None => false,
        }
    }

    // Heuristically determine whether a frame is likely to be part of language
    // runtime.
    fn is_runtime_init_code(&self) -> bool {
        const SYM_PREFIXES: &[&str] =
            &["std::rt::lang_start::", "test::run_test::run_test_inner::"];

        let (name, file) = match (self.name.as_ref(), self.filename.as_ref()) {
            (Some(name), Some(filename)) => (name, filename.to_string_lossy()),
            _ => return false,
        };

        if SYM_PREFIXES.iter().any(|x| name.starts_with(x)) {
            return true;
        }

        // For Linux, this is the best rule for skipping test init I found.
        if name == "{{closure}}" && file == "src/libtest/lib.rs" {
            return true;
        }

        false
    }

    fn print_source_if_avail(&self, s: &mut Settings) -> IOResult {
        let (lineno, filename) = match (self.lineno, self.filename.as_ref()) {
            (Some(a), Some(b)) => (a, b),
            // Without a line number and file name, we can't sensibly proceed.
            _ => return Ok(()),
        };

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
                s.out.attr(Attr::Bold)?;
                writeln!(s.out, "{:>8} > {}", cur_line_no, line?)?;
                s.out.reset()?;
            } else {
                writeln!(s.out, "{:>8} │ {}", cur_line_no, line?)?;
            }
        }

        Ok(())
    }

    fn print(&self, i: usize, s: &mut Settings) -> IOResult {
        let is_dependency_code = self.is_dependency_code();

        // Print frame index.
        write!(s.out, "{:>2}: ", i)?;

        let name = self
            .name
            .as_ref()
            .map(String::as_str)
            .unwrap_or("<unknown>");

        // Does the function have a hash suffix?
        // (dodging a dep on the regex crate here)
        let has_hash_suffix = name.len() > 19
            && &name[name.len() - 19..name.len() - 16] == "::h"
            && name[name.len() - 16..].chars().all(|x| x.is_digit(16));

        // Print function name.
        s.out.fg(if is_dependency_code {
            color::GREEN
        } else {
            color::BRIGHT_RED
        })?;

        if has_hash_suffix && s.dim_function_hash_part {
            write!(s.out, "{}", &name[..name.len() - 19])?;
            s.out.fg(color::BRIGHT_BLACK)?;
            writeln!(s.out, "{}", &name[name.len() - 19..])?;
        } else {
            writeln!(s.out, "{}", name)?;
        }

        s.out.reset()?;

        // Print source location, if known.
        if let Some(ref file) = self.filename {
            let filestr = file.to_str().unwrap_or("<bad utf8>");
            let lineno = self
                .lineno
                .map_or("<unknown line>".to_owned(), |x| x.to_string());
            writeln!(s.out, "    at {}:{}", filestr, lineno)?;
        } else {
            writeln!(s.out, "    at <unknown source file>")?;
        }

        // Maybe print source.
        if s.verbosity >= Verbosity::Full {
            self.print_source_if_avail(s)?;
        }

        Ok(())
    }
}

// ============================================================================================== //
// [Settings]                                                                                     //
// ============================================================================================== //

/// Configuration for panic printing.
pub struct Settings {
    message: String,
    out: Box<dyn PanicOutputStream>,
    verbosity: Verbosity,
    dim_function_hash_part: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let term = term::stderr();

        Self {
            verbosity: Verbosity::from_env(),
            message: "The application panicked (crashed).".to_owned(),
            out: if term.is_some() && atty::is(atty::Stream::Stderr) {
                Box::new(ColorizedStderrOutput::new(term.unwrap()))
            } else {
                Box::new(StreamOutput::new(std::io::stderr()))
            },
            dim_function_hash_part: true,
        }
    }
}

impl fmt::Debug for Settings {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Settings")
            .field("message", &self.message)
            .field("verbosity", &self.verbosity)
            .field("dim_function_hash_part", &self.dim_function_hash_part)
            .finish()
    }
}

impl Settings {
    /// Alias for `Settings::default`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Controls the "greeting" message of the panic.
    ///
    /// Defaults to `"The application panicked (crashed)"`.
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    /// Controls where output is directed to.
    ///
    /// Defaults to colorized output to `stderr` when attached to a tty
    /// or colorless output when not.
    pub fn output_stream(mut self, out: Box<dyn PanicOutputStream>) -> Self {
        self.out = out;
        self
    }

    /// Controls the verbosity level.
    ///
    /// Defaults to `Verbosity::get_env()`.
    pub fn verbosity(mut self, v: Verbosity) -> Self {
        self.verbosity = v;
        self
    }

    /// Controls whether the hash part of functions is printed dimmed.
    ///
    /// Defaults to `true`.
    pub fn dim_function_hash_part(mut self, dim: bool) -> Self {
        self.dim_function_hash_part = dim;
        self
    }
}

// ============================================================================================== //
// [Term output abstraction]                                                                      //
// ============================================================================================== //

/// Colorization subset of `term::Terminal` trait.
pub trait Colorize {
    fn fg(&mut self, color: color::Color) -> IOResult;
    fn bg(&mut self, color: color::Color) -> IOResult;
    fn attr(&mut self, attr: Attr) -> IOResult;
    fn reset(&mut self) -> IOResult;
}

/// Combined `Colorize + Write + Send` trait, for usage with `Box`.
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

    fn attr(&mut self, attr: Attr) -> IOResult {
        Ok(self.term.attr(attr)?)
    }

    fn reset(&mut self) -> IOResult {
        Ok(self.term.reset()?)
    }
}

impl Write for ColorizedStderrOutput {
    fn write(&mut self, buf: &[u8]) -> IOResult<usize> {
        self.term.get_mut().write(buf)
    }

    fn flush(&mut self) -> IOResult {
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
    fn write(&mut self, buf: &[u8]) -> IOResult<usize> {
        self.stream.write(buf)
    }

    fn flush(&mut self) -> IOResult {
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

    fn attr(&mut self, _attr: Attr) -> IOResult {
        Ok(())
    }

    fn reset(&mut self) -> IOResult {
        Ok(())
    }
}

impl<T: Write + Send> PanicOutputStream for StreamOutput<T> {}

// ============================================================================================== //
// [Panic printing]                                                                               //
// ============================================================================================== //

fn print_backtrace(s: &mut Settings) -> IOResult {
    writeln!(s.out, "{:━^80}", " BACKTRACE ")?;

    // Collect frame info.
    let frames: Vec<_> = backtrace::Backtrace::new()
        .frames()
        .iter()
        .flat_map(|frame| frame.symbols())
        .map(|sym| Frame {
            name: sym.name().map(|x| x.to_string()),
            lineno: sym.lineno(),
            filename: sym.filename().map(|x| x.into()),
        })
        .collect();

    // Try to find where the interesting part starts...
    let top_cutoff = frames
        .iter()
        .rposition(Frame::is_post_panic_code)
        .map(|x| x + 1)
        .unwrap_or(0);

    // Try to find where language init frames start ...
    let bottom_cutoff = frames
        .iter()
        .position(Frame::is_runtime_init_code)
        .unwrap_or_else(|| frames.len());

    if top_cutoff != 0 {
        let text = format!("({} post panic frames hidden)", top_cutoff);
        s.out.fg(color::BRIGHT_CYAN)?;
        writeln!(s.out, "{:^80}", text)?;
        s.out.reset()?;
    }

    // Turn them into `Frame` objects and print them.
    let num_frames = frames.len();
    let frames = frames
        .into_iter()
        .skip(top_cutoff)
        .take(bottom_cutoff - top_cutoff)
        .zip(top_cutoff..);

    // Print surviving frames.
    for (frame, i) in frames {
        frame.print(i, s)?;
    }

    if bottom_cutoff != num_frames {
        let text = format!(
            "({} runtime init frames hidden)",
            num_frames - bottom_cutoff
        );
        s.out.fg(color::BRIGHT_CYAN)?;
        writeln!(s.out, "{:^80}", text)?;
        s.out.reset()?;
    }

    Ok(())
}

fn print_panic_info(pi: &PanicInfo, s: &mut Settings) -> IOResult {
    s.out.fg(color::RED)?;
    writeln!(s.out, "{}", s.message)?;
    s.out.reset()?;

    // Print panic message.
    let payload = pi
        .payload()
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| pi.payload().downcast_ref::<&str>().cloned())
        .unwrap_or("<non string panic payload>");

    write!(s.out, "Message:  ")?;
    s.out.fg(color::CYAN)?;
    writeln!(s.out, "{}", payload)?;
    s.out.reset()?;

    // If known, print panic location.
    write!(s.out, "Location: ")?;
    if let Some(loc) = pi.location() {
        s.out.fg(color::MAGENTA)?;
        write!(s.out, "{}", loc.file())?;
        s.out.fg(color::WHITE)?;
        write!(s.out, ":")?;
        s.out.fg(color::MAGENTA)?;
        writeln!(s.out, "{}", loc.line())?;
        s.out.reset()?;
    } else {
        writeln!(s.out, "<unknown>")?;
    }

    // Print some info on how to increase verbosity.
    if s.verbosity == Verbosity::Minimal {
        write!(s.out, "\nBacktrace omitted. Run with ")?;
        s.out.attr(Attr::Bold)?;
        write!(s.out, "RUST_BACKTRACE=1")?;
        s.out.reset()?;
        writeln!(s.out, " environment variable to display it.")?;
    }
    if s.verbosity <= Verbosity::Medium {
        if s.verbosity == Verbosity::Medium {
            // If exactly medium, no newline was printed before.
            writeln!(s.out)?;
        }

        write!(s.out, "Run with ")?;
        s.out.attr(Attr::Bold)?;
        write!(s.out, "RUST_BACKTRACE=full")?;
        s.out.reset()?;
        writeln!(s.out, " to include source snippets.")?;
    }

    if s.verbosity >= Verbosity::Medium {
        print_backtrace(s)?;
    }

    Ok(())
}

// ============================================================================================== //
