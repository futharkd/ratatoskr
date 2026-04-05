# mimir

`mimir` is the shared configuration library for the yggdrasil workspace.

## Scope

- Canonical config schema types (`AppConfig` and related structs)
- Main config loading from TOML
- Split-file includes (convention folders + explicit globs)
- Merge and duplicate validation rules
- Default value application

## Consumer

`ratatoskr` imports `mimir::config` for all config types and loading behavior.
