fn main() {
    use color_backtrace::{install_with_settings, Settings};
    install_with_settings(Settings::new().message("Custom message!"));
    assert_eq!(1, 2);
}
