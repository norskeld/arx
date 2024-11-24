# decaff

[![Checks](https://img.shields.io/github/actions/workflow/status/norskeld/decaff/checks.yml?style=flat-square&colorA=22272d&colorB=22272d&label=checks)](https://github.com/norskeld/decaff/actions)

Opinionated, simple, user-friendly command-line tool for declarative scaffolding.

## Status

> [!NOTE]
>
> Mostly working, but bugs and uncovered edge cases to be expected.

## Installation

Right now **decaff** can only be installed from source via **Cargo**.

### From source (Cargo)

Make sure to [install Rust toolchain][rust-toolchain] first. After that you can install decaff using **Cargo**:

```shell
cargo install --locked --git https://github.com/norskeld/decaff
```

## Example

Below is a sample configuration file that demonstrates features of **decaff** and can be used as a reference.

```scala
// Options defined here can be overridden from CLI.
options {
  // Delete decaff config file after we're done. Defaults to `true`.
  delete false
}

// Actions to run after the repository was successfully downloaded and unpacked. All actions or
// suites of actions run sequentially, there is no concurrency or out-of-order execution for
// predictable outcomes.
//
// You can define either suites of actions — named groups of actions — or a flat list of actions,
// but not both.
//
// Notes:
//
// - Unpacking into an existing destination is forbidden.
// - Invalid or unknown actions, nodes or replacements will be skipped. Warnings will be issued.
// - Action failure terminates the main process.
// - No cleanup on failures by default.
actions {
  suite "hello" {
    // This action simply echoes the argument to stdout. Raw strings are trimmed by default and
    // aligned to the leftmost non-whitespace character. Trimming can be disabled with `trim=false`.
    echo r#"
      Sup! Let's set everything up. We will:

      - Print this message.
      - Ask some questions via prompts.
      - Initialize git repository (not for real).
      - Run some commands that will use input from prompts.
      - Commit everything (again, not for real).
    "#
  }

  // In this suite we run a series of prompts asking different questions.
  //
  // Answers will be stored globally and available from any _subsequent_ action or suite of actions.
  suite "prompts" {
    // Text prompt.
    input "repo_name" {
      hint "Repository name"
      default "norskeld/serpent"
    }

    // Editor prompt. This runs the default $EDITOR.
    editor "repo_desc" {
      hint "Repository description"
      default "Scaffolded with decaff"
    }

    // Select prompt.
    select "repo_pm" {
      hint "Package manager of choice"
      options "npm" "pnpm" "yarn" "bun"
    }

    // Number prompt. Accepts both integers and floats.
    number "magic_number" {
      hint "Magic number"
      default 42
    }

    // If no default value provided, prompt will become required.
    input "repo_pm_args" {
      hint "Additional arguments for package manager"
    }

    // Simple confirm prompt.
    confirm "should_commit" {
      hint "Whether to stage and commit changes after scaffolding"
      default false
    }
  }

  suite "git" {
    // This action runs a given shell command and prints its output to stdout.
    run "echo git init"
  }

  // Here we demonstrate using replacements.
  suite "replacements" {
    // Replace all occurences of given replacements in files that match the glob pattern.
    replace in=".template/**" {
      "repo_name"
      "repo_desc"
    }

    // Replace all occurences of given replacements in _all_ files. This is equivalent to using
    // "**/*" as the glob pattern.
    replace {
      "repo_pm"
    }

    // Trying to run a non-existent replacement will do nothing (a warning will be issued though).
    replace {
      "NONEXISTENTREPLACEMENT"
    }
  }

  // In this suite we demonstrate actions for operating on files. All these actions support glob
  // patterns, except the `to` field, that should be a relative path.
  //
  // Note:
  //
  // - Paths don't expand, i.e. ~ won't expand to the home directory and env vars won't work either.
  suite "files" {
    cp from=".template/*.toml" to="."
    rm ".template/*.toml"
    mv from=".template/**/*" to="."
    rm ".template"
  }

  // Here we demonstrate how to inject prompts' values.
  suite "install" {
    // To disambiguate whether {repo_pm} is part of a command or is a replacement that should be
    // replaced with something, we pass `inject` node that explicitly tells decaff what to inject
    // into the command.
    //
    // All replacements are processed _before_ running a command.
    run "{repo_pm} install {repo_pm_args}" {
      inject "repo_pm" "repo_pm_args"
    }
  }

  // Here we demonstrate multiline commands using `run`.
  suite "commit" {
    // Similarly to the `echo` action you can use raw strings to define multiline commands. Plus,
    // you don't have to escape anything.
    //
    // The action below will be executed as if it were two separate `run` actions:
    //
    // run "git add ."
    // run "git commit -m 'chore: init repository'"
    //
    // You can name `run` actions for clarity, otherwise decaff will use either the command itself
    // or the first line of a multiline command as the hint.
    run name="stage and commit" r#"
      echo git add .
      echo git commit -m 'chore: init repository'
    "#
  }
}
```

## Acknowledgements

Thanks to [Rich Harris][rich-harris] and his [degit] for inspiration. `:^)`

## License

[MIT](LICENSE)

<!-- Links. -->

[degit]: https://github.com/Rich-Harris/degit
[rich-harris]: https://github.com/Rich-Harris
[rust-toolchain]: https://rust-lang.org/tools/install
