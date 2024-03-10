# arx

[![Checks](https://img.shields.io/github/actions/workflow/status/norskeld/arx/checks.yml?style=flat-square&colorA=22272d&colorB=22272d&label=checks)](https://github.com/norskeld/arx/actions)

Simple and user-friendly command-line tool for declarative scaffolding.

## Status

> [!NOTE]
>
> This is an MVP.
>
> - [Spec] was fleshed out and implemented, but there's no thorough test coverage yet.
> - [Spec] is thoroughly commented and temporarily serves as a reference.
> - Bugs and uncovered edge cases are to be expected.

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
