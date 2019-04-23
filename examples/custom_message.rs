fn main() {
    color_backtrace::install_with_custom_message("\"Professional\" message.");
    assert_eq!(1, 2);
}
