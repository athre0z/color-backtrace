use color_traceback;
use std::panic::PanicInfo;

fn fn4() {
    Err::<(), ()>(()).unwrap();
}

fn fn3() {
    fn4();
}

fn fn2() {
    // sdfsdf
    let _dead = 1 + 4;
    fn3();
    let _fsdf = "sdfsdf";
    2 + 5;
}

fn fn1() {
    fn2();
}

fn main() {
    std::panic::set_hook(Box::new(color_traceback::panic));
    fn1();
}
