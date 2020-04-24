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
//! use color_backtrace::{install_with_settings, Settings};
//! install_with_settings(Settings::new().message("Custom message!"));
//! ```
//!
//! ### Controlling verbosity
//! The default verbosity is configured via the `RUST_BACKTRACE` environment
//! variable. An unset `RUST_BACKTRACE` corresponds to
//! [minimal](Verbosity::Minimal), `RUST_BACKTRACE=1` to
//! [medium](Verbosity::Medium) and `RUST_BACKTRACE=full` to
//! [full](Verbosity::Full) verbosity levels.

use backtrace;
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::panic::PanicInfo;
use std::path::PathBuf;
use std::sync::Mutex;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

#[cfg(feature = "failure-bt")]
pub mod failure;

// Re-export termcolor so users don't have to depend on it themselves.
pub use termcolor;

// ============================================================================================== //
// [Result / Error types]                                                                         //
// ============================================================================================== //

type IOResult<T = ()> = Result<T, std::fmt::Error>;

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
        eprintln!("{}", print_panic_info(pi, &mut *settings_lock));
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

    fn print_source_if_avail(&self, s: &Settings, f: &mut fmt::Formatter<'_>) -> IOResult {
        let (lineno, filename) = match (self.lineno, self.filename.as_ref()) {
            (Some(a), Some(b)) => (a, b),
            // Without a line number and file name, we can't sensibly proceed.
            _ => return Ok(()),
        };

        let file = match File::open(filename) {
            Ok(file) => file,
            Err(ref e) if e.kind() == ErrorKind::NotFound => return Ok(()),
            e @ Err(_) => e.unwrap(),
        };

        // Extract relevant lines.
        let reader = BufReader::new(file);
        let start_line = lineno - 2.min(lineno - 1);
        let surrounding_src = reader.lines().skip(start_line as usize - 1).take(5);
        for (line, cur_line_no) in surrounding_src.zip(start_line..) {
            if cur_line_no == lineno {
                // Print actual source line with brighter color.
                // f.set_color(&s.colors.selected_src_ln)?;
                writeln!(f, "{:>8} > {}", cur_line_no, line.unwrap())?;
            // f.reset()?;
            } else {
                writeln!(f, "{:>8} │ {}", cur_line_no, line.unwrap())?;
            }
        }

        Ok(())
    }

    fn print(&self, i: usize, s: &Settings, f: &mut fmt::Formatter<'_>) -> IOResult {
        let is_dependency_code = self.is_dependency_code();

        // Print frame index.
        write!(f, "{:>2}: ", i)?;

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
        // f.set_color(if is_dependency_code {
        //     &s.colors.dependency_code
        // } else {
        //     &s.colors.crate_code
        // })?;

        if has_hash_suffix {
            write!(f, "{}", &name[..name.len() - 19])?;
            if s.strip_function_hash {
                writeln!(f)?;
            } else {
                // f.set_color(if is_dependency_code {
                // &s.colors.dependency_code_hash
                // } else {
                // &s.colors.crate_code_hash
                // })?;
                writeln!(f, "{}", &name[name.len() - 19..])?;
            }
        } else {
            writeln!(f, "{}", name)?;
        }

        // f.reset()?;

        // Print source location, if known.
        if let Some(ref file) = self.filename {
            let filestr = file.to_str().unwrap_or("<bad utf8>");
            let lineno = self
                .lineno
                .map_or("<unknown line>".to_owned(), |x| x.to_string());
            writeln!(f, "    at {}:{}", filestr, lineno)?;
        } else {
            writeln!(f, "    at <unknown source file>")?;
        }

        // Maybe print source.
        if s.verbosity >= Verbosity::Full {
            self.print_source_if_avail(s, f)?;
        }

        Ok(())
    }
}

// ============================================================================================== //
// [Settings]                                                                                     //
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

/// Configuration for panic printing.
pub struct Settings {
    message: String,
    out: Box<dyn WriteColor + Send>,
    verbosity: Verbosity,
    strip_function_hash: bool,
    colors: ColorScheme,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            verbosity: Verbosity::from_env(),
            message: "The application panicked (crashed).".to_owned(),
            out: Box::new(StandardStream::stderr(if atty::is(atty::Stream::Stderr) {
                ColorChoice::Always
            } else {
                ColorChoice::Never
            })),
            strip_function_hash: false,
            colors: ColorScheme::classic(),
        }
    }
}

impl fmt::Debug for Settings {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Settings")
            .field("message", &self.message)
            .field("verbosity", &self.verbosity)
            .field("strip_function_hash_part", &self.strip_function_hash)
            .field("colors", &self.colors)
            .finish()
    }
}

impl Settings {
    /// Alias for `Settings::default`.
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

    /// Controls where output is directed to.
    ///
    /// Defaults to colorized output to `stderr` when attached to a tty
    /// or colorless output when not.
    pub fn output_stream(mut self, out: Box<dyn WriteColor + Send>) -> Self {
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

    /// Controls whether the hash part of functions is printed stripped.
    ///
    /// Defaults to `false`.
    pub fn strip_function_hash(mut self, strip: bool) -> Self {
        self.strip_function_hash = strip;
        self
    }
}

// ============================================================================================== //
// [Panic printing]                                                                               //
// ============================================================================================== //

struct BacktracePrinter<'a> {
    trace: &'a backtrace::Backtrace,
    settings: &'a Settings,
}

impl<'a> fmt::Display for BacktracePrinter<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{:━^80}", " BACKTRACE ")?;

        // Collect frame info.
        let frames: Vec<_> = self
            .trace
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
            // f.set_color(&s.colors.frames_omitted_msg)?;
            writeln!(f, "{:^80}", text)?;
            // f.reset()?;
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
            frame.print(i, self.settings, f)?;
        }

        if bottom_cutoff != num_frames {
            let text = format!(
                "({} runtime init frames hidden)",
                num_frames - bottom_cutoff
            );
            // f.set_color(&s.colors.frames_omitted_msg)?;
            writeln!(f, "{:^80}", text)?;
            // f.reset()?;
        }

        Ok(())
    }
}

/// Pretty-prints a [`backtrace::Backtrace`](backtrace::Backtrace) according the
/// the given settings.
pub fn print_backtrace<'a>(
    trace: &'a backtrace::Backtrace,
    settings: &'a Settings,
) -> impl fmt::Display + 'a {
    BacktracePrinter { trace, settings }
}

struct PanicPrinter<'a> {
    pi: &'a PanicInfo<'a>,
    s: &'a mut Settings,
}

impl<'a> fmt::Display for PanicPrinter<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let pi = self.pi;

        // f.set_color(&s.colors.header)?;
        writeln!(f, "{}", self.s.message)?;
        // f.reset()?;

        // Print panic message.
        let payload = pi
            .payload()
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| pi.payload().downcast_ref::<&str>().cloned())
            .unwrap_or("<non string panic payload>");

        write!(f, "Message:  ")?;
        // f.set_color(&s.colors.msg_loc_prefix)?;
        writeln!(f, "{}", payload)?;
        // f.reset()?;

        // If known, print panic location.
        write!(f, "Location: ")?;
        if let Some(loc) = pi.location() {
            // f.set_color(&s.colors.src_loc)?;
            write!(f, "{}", loc.file())?;
            // f.set_color(&s.colors.src_loc_separator)?;
            write!(f, ":")?;
            // f.set_color(&s.colors.src_loc)?;
            writeln!(f, "{}", loc.line())?;
        // f.reset()?;
        } else {
            writeln!(f, "<unknown>")?;
        }

        // Print some info on how to increase verbosity.
        if self.s.verbosity == Verbosity::Minimal {
            write!(f, "\nBacktrace omitted. Run with ")?;
            // f.set_color(&s.colors.env_var)?;
            write!(f, "RUST_BACKTRACE=1")?;
            // f.reset()?;
            writeln!(f, " environment variable to display it.")?;
        }
        if self.s.verbosity <= Verbosity::Medium {
            if self.s.verbosity == Verbosity::Medium {
                // If exactly medium, no newline was printed before.
                writeln!(f)?;
            }

            write!(f, "Run with ")?;
            // f.set_color(&s.colors.env_var)?;
            write!(f, "RUST_BACKTRACE=full")?;
            // f.reset()?;
            writeln!(f, " to include source snippets.")?;
        }

        if self.s.verbosity >= Verbosity::Medium {
            write!(
                f,
                "{}",
                print_backtrace(&backtrace::Backtrace::new(), self.s)
            )?;
        }

        Ok(())
    }
}

/// Pretty-prints a [`PanicInfo`](PanicInfo) struct according to the given
/// settings.
pub fn print_panic_info<'a>(pi: &'a PanicInfo, s: &'a mut Settings) -> impl fmt::Display + 'a {
    PanicPrinter { pi, s }
}

// ============================================================================================== //
