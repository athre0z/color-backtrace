use color_backtrace::PanicPrinter;

fn main() -> Result<(), std::io::Error> {
    let trace = backtrace::Backtrace::new();
    let printer = PanicPrinter::default();
    let str = printer.print_trace_to_string(&trace)?;

    if cfg!(windows) {
        println!(
            "Warning: on Windows, you'll have to enable VT100 \
             printing for your app in order for this to work \
             correctly. This example doesn't do this."
        );
    }

    println!("{}", str);

    Ok(())
}
