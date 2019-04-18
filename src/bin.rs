use color_traceback;

fn fn4() {
    color_traceback::panic(unsafe { std::mem::uninitialized() });
    //Err::<(), ()>(()).unwrap();
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
    //std::panic::set_hook(Box::new(color_traceback::panic));
    fn1();
}
