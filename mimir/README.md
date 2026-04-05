# mimir

`mimir` is the shared configuration library for the yggdrasil workspace.

## Scope

- Standalone `mimir` section primitives (`MimirConfig`)
- Shared placeholder parsing/resolution (`{env:VAR}`, `{file:/abs/path}`)
- Policy merge helper via `PlaceholderOverride`
- Consumer default merging helper (`MimirConfig::with_fallbacks`)
- Generic TOML loader utility (`load_toml_file`)

`mimir` does not define application schemas like providers/services/jobs.
Each consumer crate owns its schema and can embed:

```toml
[mimir.placeholders]
env = true
file = false
```

Then the consumer decides its own defaults and override hierarchy.
