use color_backtrace::{default_output_stream, BacktracePrinter};

fn main() {
    BacktracePrinter::new()
        .message("Custom message!")
        .install(default_output_stream());
    assert_eq!(1, 2);
}
