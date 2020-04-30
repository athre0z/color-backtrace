use color_backtrace::BacktracePrinter;

fn main() -> Result<(), std::io::Error> {
    let trace = backtrace::Backtrace::new();
    let printer = BacktracePrinter::default();
    let str = printer.format_trace_to_string(&trace)?;

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
