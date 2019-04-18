use backtrace;
use std::fs::File;
use std::io::BufReader;
use std::io::{BufRead, ErrorKind};
use std::panic::PanicInfo;
use std::path::{Path, PathBuf};
use term::{self, color, StderrTerminal};

type IOResult<T = ()> = Result<T, std::io::Error>;

#[derive(Debug)]
struct Sym {
    name: Option<String>,
    lineno: Option<u32>,
    filename: Option<PathBuf>,
}

static BUILTIN_PREFIXES: &[&str] = &[
    "std::",
    "core::",
    "backtrace::backtrace::",
    "_rust_begin_unwind",
    "color_traceback::",
    "___rust_maybe_catch_panic",
    "_main",
];

impl Sym {
    fn is_builtin(&self) -> bool {
        match self.name {
            Some(ref name) => BUILTIN_PREFIXES.iter().any(|x| name.starts_with(x)),
            None => false,
        }
    }

    pub fn print_source_if_avail(&self, t: &mut StderrTerminal) -> IOResult {
        let (lineno, filename) = match (self.lineno, self.filename.as_ref()) {
            (Some(a), Some(b)) => (a as usize - 1, b),
            // Without a line number and file name, we can't sensibly proceed.
            _ => return Ok(()),
        };

        print_source_if_avail(lineno, filename, t)
    }

    pub fn print_loc(&self, i: usize, t: &mut StderrTerminal) -> IOResult {
        let is_builtin = self.is_builtin();

        // Print frame index.
        write!(t, "{:>2}: ", i)?;

        // Print function name, if known.
        let name_fallback = "<unknown>".to_owned();
        let name = self.name.as_ref().unwrap_or(&name_fallback);
        if is_builtin {
            t.fg(color::GREEN)?;
        } else {
            t.fg(color::BRIGHT_RED)?;
            //t.bg(color::GREEN)?;
        }
        writeln!(t, "{}", name)?;
        t.reset()?;

        // Print source location, if known.
        t.fg(color::MAGENTA)?;
        if let Some(ref file) = self.filename {
            let filestr = file.to_str().unwrap_or("<bad utf8>");
            let lineno = self
                .lineno
                .map_or("<unknown line>".to_owned(), |x| x.to_string());
            writeln!(t, "    {}:{}", filestr, lineno)?;
        } else {
            writeln!(t, "    <unknown source file>")?;
        }
        t.reset()?;

        // Print source.
        self.print_source_if_avail(t)?;

        Ok(())
    }
}

fn print_source_if_avail(lineno: usize, filename: &Path, t: &mut StderrTerminal) -> IOResult {
    let file = match File::open(filename) {
        Ok(file) => file,
        Err(ref e) if e.kind() == ErrorKind::NotFound => return Ok(()),
        e @ Err(_) => e?,
    };

    // Extract relevant lines.
    let reader = BufReader::new(file);
    let start_line = lineno - 2.min(lineno);
    let surrounding_src = reader.lines().skip(start_line).take(5);
    for (line, cur_line_no) in surrounding_src.zip(start_line..) {
        if cur_line_no == lineno {
            // Print actual source line with brighter color.
            t.fg(color::BRIGHT_WHITE)?;
            writeln!(t, ">>{:>6} {}", cur_line_no, line?)?;
            t.reset()?;
        } else {
            writeln!(t, "{:>8} {}", cur_line_no, line?)?;
        }
    }

    Ok(())
}

fn print_backtrace(t: &mut StderrTerminal) -> IOResult {
    writeln!(t, "\n{:-^80}", "[ BACKTRACE ]")?;

    let mut i = 0;
    let mut result = Ok(());
    backtrace::trace(|x| {
        println!();

        let mut symbol = None;
        backtrace::resolve(x.ip(), |sym| {
            debug_assert!(symbol.is_none());
            symbol = Some(Sym {
                name: sym.name().map(|x| x.to_string()),
                lineno: sym.lineno(),
                filename: sym.filename().map(|x| x.into()),
            });
        });

        if let Some(sym) = symbol.as_ref() {
            match sym.print_loc(i, t) {
                Ok(_) => (),
                Err(e) => {
                    result = Err(e);
                    return false;
                }
            }
        }

        i += 1;
        true
    });

    Ok(())
}

fn print_panic_info(t: &mut StderrTerminal, pi: &PanicInfo) -> IOResult {
    t.fg(color::RED)?;
    writeln!(t, "\nOh noez! Panic! 💥\n")?;
    t.reset()?;

    let payload_fallback = "<non string panic payload>".to_owned();
    let payload: &String = pi.payload().downcast_ref().unwrap_or(&payload_fallback);
    write!(t, "Panic message: ")?;
    t.fg(color::CYAN)?;
    writeln!(t, "{}", payload)?;
    t.reset()?;

    Ok(())
}

pub fn panic(pi: &PanicInfo) {
    let mut t = term::stderr().unwrap();
    print_panic_info(&mut *t, pi).unwrap();
    print_backtrace(&mut *t).unwrap();
}
