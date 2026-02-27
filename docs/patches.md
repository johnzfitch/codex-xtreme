# Patch Files

`codex-xtreme` loads patch definitions from:
- `~/dev/codex-patcher/patches` (development layout)
- `~/.config/codex-patcher/patches` (installed layout)
- `CODEX_PATCHER_PATCHES` (explicit override)

## Recommended Patch Files

| Patch File | Version Range | Link |
|------------|---------------|------|
| `privacy.toml` | `>=0.88.0, <0.99.0-alpha.7` | [view](https://github.com/johnzfitch/codex-patcher/blob/main/patches/privacy.toml) |
| `privacy-v0.99-alpha1-alpha22.toml` | `>=0.99.0-alpha.10, <0.99.0-alpha.14` | [view](https://github.com/johnzfitch/codex-patcher/blob/main/patches/privacy-v0.99-alpha1-alpha22.toml) |
| `privacy-v0.99-alpha14-alpha20.toml` | `>=0.99.0-alpha.14, <0.99.0-alpha.21` | [view](https://github.com/johnzfitch/codex-patcher/blob/main/patches/privacy-v0.99-alpha14-alpha20.toml) |
| `privacy-v0.99-alpha23.toml` | `>=0.99.0-alpha.21, <0.105.0-alpha.13` | [view](https://github.com/johnzfitch/codex-patcher/blob/main/patches/privacy-v0.99-alpha23.toml) |
| `privacy-v0.105-alpha13.toml` | `>=0.105.0-alpha.13, <0.106.0` | [view](https://github.com/johnzfitch/codex-patcher/blob/main/patches/privacy-v0.105-alpha13.toml) |
| `memory-safety-regressions.toml` | `>=0.101.0-alpha.1, <0.102.0` | [view](https://github.com/johnzfitch/codex-patcher/blob/main/patches/memory-safety-regressions.toml) |

## Install Additional Patch Files

```bash
mkdir -p ~/.config/codex-patcher/patches
curl -fsSL https://raw.githubusercontent.com/johnzfitch/codex-patcher/main/patches/privacy-v0.99-alpha1-alpha22.toml -o ~/.config/codex-patcher/patches/privacy-v0.99-alpha1-alpha22.toml
curl -fsSL https://raw.githubusercontent.com/johnzfitch/codex-patcher/main/patches/privacy-v0.99-alpha14-alpha20.toml -o ~/.config/codex-patcher/patches/privacy-v0.99-alpha14-alpha20.toml
curl -fsSL https://raw.githubusercontent.com/johnzfitch/codex-patcher/main/patches/privacy-v0.99-alpha23.toml -o ~/.config/codex-patcher/patches/privacy-v0.99-alpha23.toml
curl -fsSL https://raw.githubusercontent.com/johnzfitch/codex-patcher/main/patches/privacy-v0.105-alpha13.toml -o ~/.config/codex-patcher/patches/privacy-v0.105-alpha13.toml
curl -fsSL https://raw.githubusercontent.com/johnzfitch/codex-patcher/main/patches/memory-safety-regressions.toml -o ~/.config/codex-patcher/patches/memory-safety-regressions.toml
```

## Apply Through codex-xtreme

1. Run `codex-xtreme`.
2. Select a target Codex release/tag.
3. In patch selection, keep the privacy and memory safety patch files selected.
4. Continue the workflow; incompatible patch files are skipped by version constraints.
