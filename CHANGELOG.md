# Changelog
All notable changes to this project will be documented in this file.

## [v0.2.0] (2019-06-22)

### Added
- This changelog!
- Customization via settings
- Printing to streams other than stderr

### Changed
- Improved {dependency code,post panic frame,runtime init} heuristics
- Changed default panic message to be more professional
- Relicensed from MIT to MIT/Apache-2.0 dual licensing
- Moved `get_verbosity` -> `Verbosity::from_env`
- Fix readability on light terminal themes
- Fix deadlock when unwrapping an error while printing the panic
- Many internal tweaks

[v0.2.0]: https://github.com/athre0z/color-backtrace/releases/tag/v0.2.0