# Patch Files

`codex-xtreme` loads patch definitions from:
- `~/dev/codex-patcher/patches` (development layout)
- `~/.config/codex-patcher/patches` (installed layout)
- `CODEX_PATCHER_PATCHES` (explicit override)

## Core Privacy Patch Files

| Patch File | Version Range | Link |
|------------|---------------|------|
| `privacy.toml` | `>=0.88.0, <0.99.0-alpha.7` | [view](https://github.com/johnzfitch/codex-patcher/blob/main/patches/privacy.toml) |
| `privacy-v0.99-alpha1-alpha22.toml` | `>=0.99.0-alpha.10, <0.99.0-alpha.14` | [view](https://github.com/johnzfitch/codex-patcher/blob/main/patches/privacy-v0.99-alpha1-alpha22.toml) |
| `privacy-v0.99-alpha14-alpha20.toml` | `>=0.99.0-alpha.14, <0.99.0-alpha.21` | [view](https://github.com/johnzfitch/codex-patcher/blob/main/patches/privacy-v0.99-alpha14-alpha20.toml) |
| `privacy-v0.99-alpha23.toml` | `>=0.99.0-alpha.21` | [view](https://github.com/johnzfitch/codex-patcher/blob/main/patches/privacy-v0.99-alpha23.toml) |

## Install Additional v0.99 Privacy Files

```bash
mkdir -p ~/.config/codex-patcher/patches
curl -fsSL https://raw.githubusercontent.com/johnzfitch/codex-patcher/main/patches/privacy-v0.99-alpha1-alpha22.toml -o ~/.config/codex-patcher/patches/privacy-v0.99-alpha1-alpha22.toml
curl -fsSL https://raw.githubusercontent.com/johnzfitch/codex-patcher/main/patches/privacy-v0.99-alpha14-alpha20.toml -o ~/.config/codex-patcher/patches/privacy-v0.99-alpha14-alpha20.toml
curl -fsSL https://raw.githubusercontent.com/johnzfitch/codex-patcher/main/patches/privacy-v0.99-alpha23.toml -o ~/.config/codex-patcher/patches/privacy-v0.99-alpha23.toml
```

## Apply Through codex-xtreme

1. Run `codex-xtreme`.
2. Select a target Codex release/tag.
3. In patch selection, keep the privacy patch files selected.
4. Continue the workflow; incompatible patch files are skipped by version constraints.
