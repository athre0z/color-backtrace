//! Colorful and clean backtraces on panic.
//!
//! This library aims to make panics a little less painful by nicely colorizing
//! them, skipping over frames of functions called after the panic was already
//! initiated and printing relevant source snippets. Frames of functions in your
//! application are colored in a different color (red) than those of
//! dependencies (green).
//!
//! ### Screenshot
//! ![Screenshot](https://i.imgur.com/jLznHxp.png)
//!
//! ### Features
//! - Colorize backtraces to be easier on the eyes
//! - Show source snippets if source files are found on disk
//! - Print frames of application code vs dependencies in different color
//! - Hide all the frames after the panic was already initiated
//! - Hide language runtime initialization frames
//!
//! ### Installing the panic handler
//!
//! In your main function, just insert the following snippet. That's it!
//! ```rust
//! color_backtrace::install();
//! ```
//!
//! If you want to customize some settings, you can instead do:
//! ```rust
//! use color_backtrace::{default_output_stream, BacktracePrinter};
//! BacktracePrinter::new().message("Custom message!").install(default_output_stream());
//! ```
//!
//! ### Controlling verbosity
//! The default verbosity is configured via the `RUST_BACKTRACE` environment
//! variable. An unset `RUST_BACKTRACE` corresponds to
//! [minimal](Verbosity::Minimal), `RUST_BACKTRACE=1` to
//! [medium](Verbosity::Medium) and `RUST_BACKTRACE=full` to
//! [full](Verbosity::Full) verbosity levels.

use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader, ErrorKind};
use std::panic::PanicInfo;
use std::path::PathBuf;
use std::sync::Mutex;
use termcolor::{Ansi, Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

#[cfg(feature = "failure-bt")]
pub mod failure;

// Re-export termcolor so users don't have to depend on it themselves.
pub use termcolor;

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
        match read_env() {
            Some(ref x) if x == "full" => Verbosity::Full,
            Some(_) => Verbosity::Medium,
            None => Verbosity::Minimal,
        }
    }
}

fn read_env() -> Option<String> {
    env::var("RUST_LIB_BACKTRACE")
        .or_else(|_| env::var("RUST_BACKTRACE"))
        .ok()
}
// ============================================================================================== //
// [Panic handler and install logic]                                                              //
// ============================================================================================== //

/// Install a `BacktracePrinter` handler with `::default()` settings.
///
/// This currently is a convenience shortcut for writing
///
/// ```rust
/// use color_backtrace::{BacktracePrinter, default_output_stream};
/// BacktracePrinter::default().install(default_output_stream())
/// ```
pub fn install() {
    BacktracePrinter::default().install(default_output_stream());
}

/// Create the default output stream.
///
/// If stderr is attached to a tty, this is a colorized stderr, else it's
/// a plain (colorless) stderr.
pub fn default_output_stream() -> Box<StandardStream> {
    Box::new(StandardStream::stderr(if atty::is(atty::Stream::Stderr) {
        ColorChoice::Always
    } else {
        ColorChoice::Never
    }))
}

#[deprecated(
    since = "0.4",
    note = "Use `BacktracePrinter::into_panic_handler()` instead."
)]
pub fn create_panic_handler(
    printer: BacktracePrinter,
) -> Box<dyn Fn(&PanicInfo<'_>) + 'static + Sync + Send> {
    let out_stream_mutex = Mutex::new(default_output_stream());
    Box::new(move |pi| {
        let mut lock = out_stream_mutex.lock().unwrap();
        if let Err(e) = printer.print_panic_info(pi, &mut *lock) {
            // Panicking while handling a panic would send us into a deadlock,
            // so we just print the error to stderr instead.
            eprintln!("Error while printing panic: {:?}", e);
        }
    })
}

#[deprecated(since = "0.4", note = "Use `BacktracePrinter::install()` instead.")]
pub fn install_with_settings(printer: BacktracePrinter) {
    std::panic::set_hook(printer.into_panic_handler(default_output_stream()))
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

    /// Heuristically determine whether a frame is likely to be a post panic
    /// frame.
    ///
    /// Post panic frames are frames of a functions called after the actual panic
    /// is already in progress and don't contain any useful information for a
    /// reader of the backtrace.
    fn is_post_panic_code(&self) -> bool {
        const SYM_PREFIXES: &[&str] = &[
            "_rust_begin_unwind",
            "core::result::unwrap_failed",
            "core::panicking::panic_fmt",
            "color_backtrace::create_panic_handler",
            "std::panicking::begin_panic",
            "begin_panic_fmt",
            "failure::backtrace::Backtrace::new",
            "backtrace::capture",
            "failure::error_message::err_msg",
            "<failure::error::Error as core::convert::From<F>>::from",
        ];

        match self.name.as_ref() {
            Some(name) => SYM_PREFIXES.iter().any(|x| name.starts_with(x)),
            None => false,
        }
    }

    /// Heuristically determine whether a frame is likely to be part of language
    /// runtime.
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

    fn print_source_if_avail(&self, mut out: impl WriteColor, s: &BacktracePrinter) -> IOResult {
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
                out.set_color(&s.colors.selected_src_ln)?;
                writeln!(out, "{:>8} > {}", cur_line_no, line?)?;
                out.reset()?;
            } else {
                writeln!(out, "{:>8} │ {}", cur_line_no, line?)?;
            }
        }

        Ok(())
    }

    fn print(&self, i: usize, out: &mut impl WriteColor, s: &BacktracePrinter) -> IOResult {
        let is_dependency_code = self.is_dependency_code();

        // Print frame index.
        write!(out, "{:>2}: ", i)?;

        // Does the function have a hash suffix?
        // (dodging a dep on the regex crate here)
        let name = self.name.as_deref().unwrap_or("<unknown>");
        let has_hash_suffix = name.len() > 19
            && &name[name.len() - 19..name.len() - 16] == "::h"
            && name[name.len() - 16..].chars().all(|x| x.is_digit(16));

        // Print function name.
        out.set_color(if is_dependency_code {
            &s.colors.dependency_code
        } else {
            &s.colors.crate_code
        })?;

        if has_hash_suffix {
            write!(out, "{}", &name[..name.len() - 19])?;
            if s.strip_function_hash {
                writeln!(out)?;
            } else {
                out.set_color(if is_dependency_code {
                    &s.colors.dependency_code_hash
                } else {
                    &s.colors.crate_code_hash
                })?;
                writeln!(out, "{}", &name[name.len() - 19..])?;
            }
        } else {
            writeln!(out, "{}", name)?;
        }

        out.reset()?;

        // Print source location, if known.
        if let Some(ref file) = self.filename {
            let filestr = file.to_str().unwrap_or("<bad utf8>");
            let lineno = self
                .lineno
                .map_or("<unknown line>".to_owned(), |x| x.to_string());
            writeln!(out, "    at {}:{}", filestr, lineno)?;
        } else {
            writeln!(out, "    at <unknown source file>")?;
        }

        // Maybe print source.
        if s.verbosity >= Verbosity::Full {
            self.print_source_if_avail(out, s)?;
        }

        Ok(())
    }
}

// ============================================================================================== //
// [BacktracePrinter]                                                                                     //
// ============================================================================================== //

/// Color scheme definition.
#[derive(Debug)]
pub struct ColorScheme {
    pub frames_omitted_msg: ColorSpec,
    pub header: ColorSpec,
    pub msg_loc_prefix: ColorSpec,
    pub src_loc: ColorSpec,
    pub src_loc_separator: ColorSpec,
    pub env_var: ColorSpec,
    pub dependency_code: ColorSpec,
    pub dependency_code_hash: ColorSpec,
    pub crate_code: ColorSpec,
    pub crate_code_hash: ColorSpec,
    pub selected_src_ln: ColorSpec,
}

impl ColorScheme {
    /// Helper to create a new `ColorSpec` & set a few properties in one wash.
    fn cs(fg: Option<Color>, intense: bool, bold: bool) -> ColorSpec {
        let mut cs = ColorSpec::new();
        cs.set_fg(fg);
        cs.set_bold(bold);
        cs.set_intense(intense);
        cs
    }

    /// The classic `color-backtrace` scheme, as shown in the screenshots.
    pub fn classic() -> Self {
        Self {
            frames_omitted_msg: Self::cs(Some(Color::Cyan), true, false),
            header: Self::cs(Some(Color::Red), false, false),
            msg_loc_prefix: Self::cs(Some(Color::Cyan), false, false),
            src_loc: Self::cs(Some(Color::Magenta), false, false),
            src_loc_separator: Self::cs(Some(Color::White), false, false),
            env_var: Self::cs(None, false, true),
            dependency_code: Self::cs(Some(Color::Green), false, false),
            dependency_code_hash: Self::cs(Some(Color::Black), true, false),
            crate_code: Self::cs(Some(Color::Red), true, false),
            crate_code_hash: Self::cs(Some(Color::Black), true, false),
            selected_src_ln: Self::cs(None, false, true),
        }
    }
}

impl Default for ColorScheme {
    fn default() -> Self {
        Self::classic()
    }
}

#[deprecated(since = "0.4", note = "Use `BacktracePrinter` instead.")]
pub type Settings = BacktracePrinter;

/// Pretty-printer for backtraces and [`PanicInfo`](PanicInfo) structs.
pub struct BacktracePrinter {
    message: String,
    verbosity: Verbosity,
    strip_function_hash: bool,
    colors: ColorScheme,
}

impl Default for BacktracePrinter {
    fn default() -> Self {
        Self {
            verbosity: Verbosity::from_env(),
            message: "The application panicked (crashed).".to_owned(),
            strip_function_hash: false,
            colors: ColorScheme::classic(),
        }
    }
}

/// Builder functions.
impl BacktracePrinter {
    /// Alias for `BacktracePrinter::default`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Alter the color scheme.
    ///
    /// Defaults to `ColorScheme::classic()`.
    pub fn color_scheme(mut self, colors: ColorScheme) -> Self {
        self.colors = colors;
        self
    }

    /// Controls the "greeting" message of the panic.
    ///
    /// Defaults to `"The application panicked (crashed)"`.
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    /// Controls the verbosity level.
    ///
    /// Defaults to `Verbosity::get_env()`.
    pub fn verbosity(mut self, v: Verbosity) -> Self {
        self.verbosity = v;
        self
    }

    /// Controls whether the hash part of functions is printed stripped.
    ///
    /// Defaults to `false`.
    pub fn strip_function_hash(mut self, strip: bool) -> Self {
        self.strip_function_hash = strip;
        self
    }
}

/// Routines for putting the panic printer to use.
impl BacktracePrinter {
    /// Install the `color_backtrace` handler with default settings.
    ///
    /// Output streams can be created via `default_output_stream()` or
    /// using any other stream that implements
    /// [`termcolor::WriteColor`](termcolor::WriteColor).
    pub fn install(self, out: impl WriteColor + Sync + Send + 'static) {
        std::panic::set_hook(self.into_panic_handler(out))
    }

    /// Create a `color_backtrace` panic handler from this panic printer.
    ///
    /// This can be used if you want to combine the handler with other handlers.
    pub fn into_panic_handler(
        self,
        out: impl WriteColor + Sync + Send + 'static,
    ) -> Box<dyn Fn(&PanicInfo<'_>) + 'static + Sync + Send> {
        let out_stream_mutex = Mutex::new(out);
        Box::new(move |pi| {
            let mut lock = out_stream_mutex.lock().unwrap();
            if let Err(e) = self.print_panic_info(pi, &mut *lock) {
                // Panicking while handling a panic would send us into a deadlock,
                // so we just print the error to stderr instead.
                eprintln!("Error while printing panic: {:?}", e);
            }
        })
    }

    /// Pretty-prints a [`backtrace::Backtrace`](backtrace::Backtrace) to an output stream.
    pub fn print_trace(&self, trace: &backtrace::Backtrace, out: &mut impl WriteColor) -> IOResult {
        writeln!(out, "{:━^80}", " BACKTRACE ")?;

        // Collect frame info.
        let frames: Vec<_> = trace
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
            out.set_color(&self.colors.frames_omitted_msg)?;
            writeln!(out, "{:^80}", text)?;
            out.reset()?;
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
            frame.print(i, out, self)?;
        }

        if bottom_cutoff != num_frames {
            let text = format!(
                "({} runtime init frames hidden)",
                num_frames - bottom_cutoff
            );
            out.set_color(&self.colors.frames_omitted_msg)?;
            writeln!(out, "{:^80}", text)?;
            out.reset()?;
        }

        Ok(())
    }

    /// Extracts the internal `backtrace::Backtrace` from a `failure::Backtrace` and prints it, if
    /// one exists. Prints that a backtrace was not capture if one is not found.
    #[cfg(feature = "failure-bt")]
    pub unsafe fn print_failure_trace(
        &self,
        trace: &::failure::Backtrace,
        out: &mut impl WriteColor,
    ) -> IOResult {
        let internal = failure::backdoortrace(trace);

        if let Some(internal) = internal {
            self.print_trace(internal, out)
        } else {
            writeln!(out, "<failure backtrace not captured>")
        }
    }

    /// Pretty-print a backtrace to a `String`, using VT100 color codes.
    pub fn format_trace_to_string(&self, trace: &backtrace::Backtrace) -> IOResult<String> {
        // TODO: should we implicitly enable VT100 support on Windows here?
        let mut ansi = Ansi::new(vec![]);
        self.print_trace(trace, &mut ansi)?;
        Ok(String::from_utf8(ansi.into_inner()).unwrap())
    }

    /// Pretty-prints a [`PanicInfo`](PanicInfo) struct to an output stream.
    pub fn print_panic_info(&self, pi: &PanicInfo, out: &mut impl WriteColor) -> IOResult {
        out.set_color(&self.colors.header)?;
        writeln!(out, "{}", self.message)?;
        out.reset()?;

        // Print panic message.
        let payload = pi
            .payload()
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| pi.payload().downcast_ref::<&str>().cloned())
            .unwrap_or("<non string panic payload>");

        write!(out, "Message:  ")?;
        out.set_color(&self.colors.msg_loc_prefix)?;
        writeln!(out, "{}", payload)?;
        out.reset()?;

        // If known, print panic location.
        write!(out, "Location: ")?;
        if let Some(loc) = pi.location() {
            out.set_color(&self.colors.src_loc)?;
            write!(out, "{}", loc.file())?;
            out.set_color(&self.colors.src_loc_separator)?;
            write!(out, ":")?;
            out.set_color(&self.colors.src_loc)?;
            writeln!(out, "{}", loc.line())?;
            out.reset()?;
        } else {
            writeln!(out, "<unknown>")?;
        }

        // Print some info on how to increase verbosity.
        if self.verbosity == Verbosity::Minimal {
            write!(out, "\nBacktrace omitted. Run with ")?;
            out.set_color(&self.colors.env_var)?;
            write!(out, "RUST_BACKTRACE=1")?;
            out.reset()?;
            writeln!(out, " environment variable to display it.")?;
        }
        if self.verbosity <= Verbosity::Medium {
            if self.verbosity == Verbosity::Medium {
                // If exactly medium, no newline was printed before.
                writeln!(out)?;
            }

            write!(out, "Run with ")?;
            out.set_color(&self.colors.env_var)?;
            write!(out, "RUST_BACKTRACE=full")?;
            out.reset()?;
            writeln!(out, " to include source snippets.")?;
        }

        if self.verbosity >= Verbosity::Medium {
            self.print_trace(&backtrace::Backtrace::new(), out)?;
        }

        Ok(())
    }
}

// ============================================================================================== //
// [Deprecated routines for backward compat]                                                      //
// ============================================================================================== //

#[deprecated(since = "0.4", note = "Use `BacktracePrinter::print_trace` instead`")]
pub fn print_backtrace(trace: &backtrace::Backtrace, s: &mut BacktracePrinter) -> IOResult {
    s.print_trace(trace, &mut default_output_stream())
}

#[deprecated(
    since = "0.4",
    note = "Use `BacktracePrinter::print_panic_info` instead`"
)]
pub fn print_panic_info(pi: &PanicInfo, s: &mut BacktracePrinter) -> IOResult {
    s.print_panic_info(pi, &mut default_output_stream())
}

// ============================================================================================== //
