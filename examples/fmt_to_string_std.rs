use color_backtrace::BacktracePrinter;

fn main() -> Result<(), std::io::Error> {
    // Start your trace
    let trace = std::backtrace::Backtrace::force_capture();

    // Do stuff...

    // And print!
    let bt = color_backtrace::btparse::deserialize(&trace).unwrap();
    let printer = BacktracePrinter::default();
    let str = printer.format_trace_to_string(&bt)?;

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
