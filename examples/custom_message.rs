fn main() {
    use color_backtrace::{default_output_stream, install_with_settings, Settings};
    install_with_settings(
        Settings::new().message("Custom message!"),
        default_output_stream(),
    );
    assert_eq!(1, 2);
}
