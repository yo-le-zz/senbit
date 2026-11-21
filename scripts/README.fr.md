# Scripts

| Script | Rôle |
|---|---|
| `devtool.sh` | compile `devtool` (`devtool/`) et l'installe en `scripts/devtool` ; `--force` le recompile |
| `git-unify.sh` | nettoyage ponctuel : retire de l'index git les anciennes entrées de sous-modules (Linux, BusyBox, util-linux, Parted, systemd) ; les fichiers sur disque sont conservés |

Tout le reste est fait par `devtool` :

```bash
./scripts/devtool.sh            # une fois, et après toute modification des sources de devtool
./scripts/devtool doctor | install | get | build | run
```

Documentation : [🇬🇧 English](../devtool/README.md) · [🇫🇷 Français](../devtool/README.fr.md) ·
ce fichier en anglais : [README.md](README.md)
