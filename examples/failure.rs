use color_backtrace::{failure::print_backtrace, Settings};
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
            print_backtrace(&e.backtrace(), &mut Settings::new())?;
        }
    }

    Ok(())
}
