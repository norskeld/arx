# arx

[![Checks](https://img.shields.io/github/workflow/status/norskeld/arx/checks?style=flat-square&colorA=22272d&colorB=22272d&label=checks)](https://github.com/norskeld/arx/actions/workflows/checks.yml)

> `A`ugmented `R`epository E`x`tractor

Simple CLI for scaffolding projects from templates in a touch.

## Status

WIP.

## Features

`arx` allows you to make copies of git repositories, much like [degit], but with added sugar on top of its basic functionality to help scaffold projects even faster and easier.

Some of that sugar includes:

- Ability to define [replacement tags](#replacements) (aka placeholders) and simple [actions](#actions) to perform on the repository being copied. This is done via `arx.kdl` config file using the [KDL document language][kdl], which is really easy to grasp, write and read, unlike ubiquitous **JSON** and **YAML**.

- Automatically generated prompts based on the `arx.kdl` config, that will allow you to interactively replace placeholders with actual values and (optionally) run only selected actions.

## Replacements

> TODO: Document replacements.

## Actions

> TODO: Document actions.

## Acknowledgements

Thanks to [Rich Harris][rich-harris] and his [degit] tool for inspiration. `:^)`

## License

[MIT](./LICENSE)

<!-- Links. -->

[degit]: https://github.com/Rich-Harris/degit
[kdl]: https://github.com/kdl-org/kdl
[rich-harris]: https://github.com/Rich-Harris
