fn fn3() {
    fn4(); // Source printing at start the of a file ...
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

fn fn4() {
    // Source printing at the end of a file
    "x".parse::<u32>().unwrap();
}
