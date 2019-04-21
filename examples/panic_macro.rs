fn fn3() {
    panic!();
}

fn fn2() {
    fn3();
}

fn main() {
    color_backtrace::install();
    fn2();
}
