use color_backtrace::{format_backtrace, Settings};

fn main() -> Result<(), std::io::Error> {
    let trace = backtrace::Backtrace::new();
    let settings = Settings::default();
    let str = format_backtrace(&trace, &settings)?;

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
