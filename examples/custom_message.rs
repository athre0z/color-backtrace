fn main() {
    use color_backtrace::PanicPrinter;
    PanicPrinter::new().message("Custom message!").install();
    assert_eq!(1, 2);
}
