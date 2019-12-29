# Changelog
All notable changes to this project will be documented in this file.

## [v0.3.0] (2019-11-12)

#### Added
- Custom `ColorScheme` support
- Forward backtrace-rs' `gimli-symbolize` feature, which is default enabled
  - This is done by adding `default-features = false` to the `Cargo.toml`
    dependency entry for `color-backtrace`
  - Disabling it reduces transitive dependencies from ~50 → ~10
  - However, you'll pay for it with [inaccurate source info](https://github.com/athre0z/color-backtrace/issues/2) on macOS
    and Linux

#### Changed
- Replace `term` crate for colorful term printing with `termcolor`
  - This crate is more actively maintained, has fewer deps and a better API
  - This made adding color scheme support very easy
- `Settings::dim_function_hash_part` was replaced
  - Hash part color is now controlled via `ColorScheme`

#### Removed
- `Colorize`, `ColorizedStderrOutput`, `StreamOutput`, `PanicOutputStream`
  - This functionality is now all provided by the `termcolor` crate
  - `termcolor` is re-exported in the root of `color_backtrace`
- Lots of transitive dependencies!

## [v0.2.3] (2019-08-23)

#### Changed
- Added post panic frame rules for [failure]
- Updated `term` dependency

## [v0.2.2] (2019-06-30)

#### Added
- Experimental support for [failure] error backtraces
    - Contributed by Jane Lusby (@yaahallo)

#### Changed
- Switch to [gimli] backend for backtraces on macOS and Linux
    - Fixes backtraces when invoking an app outside of its build directory
- Expose `print_backtrace` and `print_panic_info` functions

## [v0.2.1] (2019-06-25)

#### Changed
- Fixed panic then `TERM` env var is not found

## [v0.2.0] (2019-06-22)

#### Added
- This changelog!
- Customization via settings
- Printing to streams other than stderr

#### Changed
- Improved {dependency code,post panic frame,runtime init} heuristics
- Changed default panic message to be more professional
- Relicensed from MIT to MIT/Apache-2.0 dual licensing
- Moved `get_verbosity` -> `Verbosity::from_env`
- Fix readability on light terminal themes
- Fix deadlock when unwrapping an error while printing the panic
- Many internal tweaks

[failure]: https://github.com/rust-lang-nursery/failure
[gimli]: https://github.com/gimli-rs/gimli

[v0.2.0]: https://github.com/athre0z/color-backtrace/releases/tag/v0.2.0
[v0.2.1]: https://github.com/athre0z/color-backtrace/releases/tag/v0.2.1
[v0.2.2]: https://github.com/athre0z/color-backtrace/releases/tag/v0.2.2
[v0.2.3]: https://github.com/athre0z/color-backtrace/releases/tag/v0.2.3
[v0.3.0]: https://github.com/athre0z/color-backtrace/releases/tag/v0.3.0

[bt-bug]: https://github.com/athre0z/color-backtrace/issues/2
