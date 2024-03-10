# arx

[![Checks](https://img.shields.io/github/workflow/status/norskeld/arx/checks?style=flat-square&colorA=22272d&colorB=22272d&label=checks)](https://github.com/norskeld/arx/actions/workflows/checks.yml)

Simple and user-friendly command-line tool for declarative scaffolding.

## Status

> [!NOTE]
>
> This is an MVP.
>
> - [Spec] was fleshed out and (mostly) implemented.
> - [Spec] is thoroughly commented and temporarily serves as a reference/documentation.
> - Bugs and uncovered edge cases are to be expected.
> - Test coverage is lacking.

## Installation

Right now **arx** can only be installed from source via **Cargo**.

### From source (Cargo)

Make sure to [install Rust toolchain][rust-toolchain] first. After that you can install arx using **Cargo**:

```shell
cargo install --locked --git https://github.com/norskeld/arx
```

## Acknowledgements

Thanks to [Rich Harris][rich-harris] and his [degit] for inspiration. `:^)`

## License

[MIT](LICENSE)

<!-- Links. -->

[spec]: spec.kdl
[degit]: https://github.com/Rich-Harris/degit
[rich-harris]: https://github.com/Rich-Harris
