use color_backtrace::{default_output_stream, PanicPrinter};
use failure::{format_err, Fallible};

fn calc_things() -> Fallible<()> {
    Err(format_err!("something went wrong"))
}

fn do_some_stuff() -> Fallible<()> {
    calc_things()
}

fn main() -> Result<(), std::io::Error> {
    if let Err(e) = do_some_stuff() {
        // oh noez!
        unsafe {
            PanicPrinter::new()
                .print_failure_trace(&e.backtrace(), &mut default_output_stream())?;
        }
    }

    Ok(())
}
