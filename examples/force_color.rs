fn fn4() {
    fn5(); // Source printing at start the of a file ...
}

fn fn3() {
    fn4();
}

fn fn2() {
    // sdfsdf
    let _dead = 1 + 4;
    fn3();
    let _fsdf = "sdfsdf";
    let _fsgg = 2 + 5;
}

fn fn1() {
    fn2();
}

fn main() {
    use color_backtrace::{install_with_settings, Settings};
    install_with_settings(Settings::new());

    fn1();
}

fn fn5() {
    // Source printing at the end of a file
    Err::<(), ()>(()).unwrap();
}
