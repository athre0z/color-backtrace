fn fn4() {
    //color_backtrace::panic(unsafe { std::mem::uninitialized() });
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
    let _fsgg = 2 + 5;
}

fn fn1() {
    fn2();
}

fn main() {
    color_backtrace::install();
    fn1();
}
