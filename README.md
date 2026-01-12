# smdc-toolchain

![Language](https://img.shields.io/github/languages/top/kidoz/smdc-toolchain)
![License](https://img.shields.io/github/license/kidoz/smdc-toolchain)

C/Rust compiler and ROM toolchain targeting the Sega Mega Drive/Genesis (Motorola 68000).

## Highlights

- `smdc` compiler: C and Rust frontends that lower to a shared IR
- M68k assembly backend plus Genesis ROM builder
- Diagnostics with source spans and IR/AST dumps for debugging

## Build

```bash
cargo build
cargo test
```

## CLI usage

```bash
smdc input.c -o output.s
smdc input.rs -o output.s
smdc input.c -o game.bin -t rom
smdc input.c -t rom --domestic-name "GAME NAME" --overseas-name "GAME NAME" -o game.bin
smdc input.c -v --dump-ast --dump-ir
```

## Architecture

```
Source Code -> Frontend -> IR -> Backend -> Output
              (C/Rust)        (M68k/ROM)
```

Key modules:

- `src/frontend/`: language frontends (C, Rust)
- `src/ir/`: shared intermediate representation
- `src/backend/`: M68k codegen + ROM builder
- `src/driver/`: pipeline orchestration
- `src/types/`: target-aware type system

## Examples

See `examples/` for minimal C and Rust programs.

## License

MIT. See `LICENSE`.
