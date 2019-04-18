use backtrace;
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

    pub fn print_loc(&self, i: usize, t: &mut StderrTerminal) -> Result<(), std::io::Error> {
        let is_builtin = self.is_builtin();

        // Print frame index.
        write!(t, "{:>4}: ", i)?;

        // Print function name, if known.
        let name_fallback = "<unknown>".to_owned();
        let name = self.name.as_ref().unwrap_or(&name_fallback);
        t.fg(if is_builtin {
            color::GREEN
        } else {
            color::BRIGHT_GREEN
        })?;
        writeln!(t, "{}", name)?;
        t.reset()?;

        // Print source location, if known.
        t.fg(if is_builtin {
            color::MAGENTA
        } else {
            color::BRIGHT_MAGENTA
        })?;

        if let Some(ref file) = self.filename {
            let filestr = file.to_str().unwrap_or("<bad utf8>");
            let lineno = self
                .lineno
                .map_or("<unknown line>".to_owned(), |x| x.to_string());
            writeln!(t, "      {}:{}", filestr, lineno)?;
        } else {
            writeln!(t, "      <unknown source file>")?;
        }

        t.reset()?;

        Ok(())
    }
}

pub fn panic(info: &PanicInfo) {
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
