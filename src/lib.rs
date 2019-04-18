use backtrace;
use std::fs::File;
use std::io::BufReader;
use std::io::{BufRead, ErrorKind};
use std::panic::PanicInfo;
use std::path::PathBuf;
use term::{self, color, StderrTerminal};

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
];

impl Sym {
    fn is_builtin(&self) -> bool {
        match self.name {
            Some(ref name) => BUILTIN_PREFIXES.iter().any(|x| name.starts_with(x)),
            None => false,
        }
    }

    pub fn print_source(&self, t: &mut StderrTerminal) -> Result<(), std::io::Error> {
        let (lineno, filename) = match (self.lineno, self.filename.as_ref()) {
            (Some(a), Some(b)) => (a as usize - 1, b),
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
        let start_line = lineno as usize - 2;
        let surrounding_src = reader.lines().skip(start_line).take(5);
        for (line, cur_line_no) in surrounding_src.zip(start_line..) {
            if cur_line_no == lineno {
                t.fg(color::BRIGHT_WHITE)?;
                writeln!(t, ">>{:>6} {}", cur_line_no, line?)?;
                t.reset()?;
            } else {
                writeln!(t, "{:>8} {}", cur_line_no, line?)?;
            }
        }

        Ok(())
    }

    pub fn print_loc(&self, i: usize, t: &mut StderrTerminal) -> Result<(), std::io::Error> {
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

        // If user code, print source.
        self.print_source(t)?;

        Ok(())
    }
}

pub fn panic(_: &PanicInfo) {
    let mut i = 0;
    let mut t = term::stderr().unwrap();

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
            sym.print_loc(i, &mut *t).unwrap();
        }

        i += 1;
        true
    });
}
