# Scripts

| Script | Role |
|---|---|
| `devtool.sh` | compiles `devtool` (`devtool/`) and installs it as `scripts/devtool`; `--force` rebuilds it |
| `git-unify.sh` | one-time cleanup: removes the old submodule entries (Linux, BusyBox, util-linux, Parted, systemd) from the git index; the files on disk are kept |

Everything else is done by `devtool`:

```bash
./scripts/devtool.sh            # once, and after changing devtool's sources
./scripts/devtool doctor | install | get | build | run
```

Documentation: [🇬🇧 English](../devtool/README.md) · [🇫🇷 Français](../devtool/README.fr.md) ·
this file in French: [README.fr.md](README.fr.md)
