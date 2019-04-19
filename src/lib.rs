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

/// Verbosity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verbosity {
    /// Print a small message including the panic payload and the panic location.
    MINIMAL,
    /// Everything in `MINIMAL` and additionally print a backtrace.
    MEDIUM,
    /// Everything in `MEDIUM` plus source snippets for all backtrace locations.
    FULL,
}

/// Retrieve verbosity level.
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

/// Panic handler printing colorful back traces.
pub fn create_panic_handler() -> Box<dyn Fn(&PanicInfo<'_>) + 'static + Sync + Send> {
    let mutex = Mutex::new(());
    Box::new(move |pi| {
        // Prevent mixed up printing when multiple threads panic at once.
        let _lock = mutex.lock();

        PanicHandler::new(pi).go().unwrap();
    })
}

/// Install the color traceback handler.
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
            "___rust_maybe_catch_panic",
            "__pthread",
            "_main",
        ];

        // Inspect name.
        if let Some(ref name) = self.name {
            if SYM_PREFIXES.iter().any(|x| name.starts_with(x)) {
                return true;
            }
        }

        // Inspect filename.
        if let Some(ref filename) = self.filename {
            let filename = filename.to_string_lossy();
            if filename.starts_with("/rustc/") || filename.contains("/.cargo/registry/src/") {
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

        for (i, (name, lineno, filename)) in symbols.into_iter().enumerate() {
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
            .map(|x| x.as_str())
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

        // Maybe print source.
        // if self.v >= Verbosity::MEDIUM {
        //     if let Some(loc) = self.pi.location() {
        //         self.print_source_if_avail(Path::new(loc.file()), loc.line() as u32)?;
        //     }
        // }

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
            v: get_verbosity(),
            pi: pi,
            t: term::stderr().unwrap(),
        }
    }
}

// ============================================================================================== //
