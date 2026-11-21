# devtool

`devtool` est l'outil de build unique de Senbit (Rust) : il vérifie la machine, télécharge les sources
**épinglées**, compile tout et démarre le système dans QEMU. Il fonctionne sous Fedora, Ubuntu/Debian et
Arch, et tous les chemins sont relatifs à la racine du projet.

[🇬🇧 English version](README.md)

## Fonctionnalités

* **Couleurs et progression** : `==>` vert pour les étapes, jaune pour « à jour / ignoré », rouge pour
  les erreurs. Les longues compilations affichent un spinner et, dessous, une ligne en direct : le
  pourcentage quand l'outil en donne un (`[12/345]`, `[ 45%]`), sinon le nombre de lignes, la durée
  écoulée et la dernière ligne. La sortie complète d'une étape en échec est enregistrée dans
  `build/logs/` et affichée.
* **Recompilation intelligente** : noyau, BusyBox, util-linux, Parted, systemd et userspace Rust ne sont
  recompilés que si quelque chose a changé (nouveau commit, patch, modification locale, sortie
  manquante). L'état est dans `build/.state/cache.json` ; supprimez-le ou passez `--force` pour tout
  recompiler.
* **Sources absentes ou vides récupérées automatiquement** : un `git clone` de Senbit laisse les
  dossiers de sources (`external/linux`, ...) vides. `devtool build` télécharge ce qui manque (à la
  version épinglée) avant de compiler ; `autogen.sh` est lancé pour util-linux quand `configure` manque
  ou est plus ancien que `configure.ac`.
* **Versions épinglées** : `config/<composant>/version` (+ `sha`/`sha256`). Rien ne suit « la dernière
  version ». Les patchs sont dans `patches/<composant>/` (voir `patches/README.md`).
* **Détection de `config/busybox.config`** : une modification est prise en compte automatiquement.
* **`--jobs`/`-j`** règle le nombre de tâches parallèles (défaut : nombre de CPU) ; **`--force`** vide le cache.
* **UEFI** : `BOOT=uefi` dans `config/vm.config` démarre QEMU avec OVMF ; l'ISO est hybride BIOS+UEFI.

## Compiler devtool

```bash
./scripts/devtool.sh            # compile devtool et l'installe en scripts/devtool
./scripts/devtool.sh --force    # recompile même si c'est à jour
```

Seul prérequis : Rust (`rustc`/`cargo`, ou [rustup](https://rustup.rs)). La racine du projet est trouvée
automatiquement (`--root CHEMIN`, `$SENBIT_ROOT`, ou en remontant depuis l'exécutable puis depuis le
dossier courant).

## Commandes

```text
devtool doctor                 vérifie la machine, affiche la commande d'installation exacte
devtool install                installe les paquets manquants (apt / dnf / pacman) ; pas en root
devtool get [composant]        télécharge les sources épinglées : all (défaut), linux, busybox,
                               util-linux, parted, systemd
devtool build [cible]          compile (voir ci-dessous)
devtool commands [list|index]  liste les commandes natives / régénère commands/INDEX.md
devtool run                    compile puis démarre dans QEMU
```

### `devtool build [cible] [-j N] [--force]`

Cibles : `all` (défaut), `fast`, `kernel`, `busybox`, `util-linux`, `parted`, `systemd`, `apt`, `rust`,
`commands`, `tools`, `rootfs`, `iso`, `package`.

`all` compile dans l'ordre : noyau → BusyBox → util-linux → Parted → systemd → apt/dpkg → userspace Rust
→ commandes natives (`commands/`, une crate par commande, détectée automatiquement : `./newcmds.sh <nom>`)
→ outils (`tools/`, keymaps et polices) → rootfs/initramfs → ISO.

* `fast` : uniquement userspace Rust, commandes, outils, rootfs et ISO (ni noyau, ni BusyBox,
  util-linux, Parted, systemd). Pour itérer vite.
* `package` : les paquets de mise à jour `senbit-kernel` et `senbit-userspace` et leur dépôt APT dans
  `build/repo/` (nécessite un `devtool build` préalable ; hors de `all`). Voir [docs/UPDATES.md](../docs/UPDATES.md).
* `apt` : apt + dpkg depuis le dépôt Debian distant (voir [docs/BUILD.md](../docs/BUILD.md)).

### `devtool run [--new] [--debug] [--fast] [--force] [-j N]`

Compile (cible `all`, ou `fast` avec `--fast`) puis démarre Senbit dans QEMU. `config/vm.config` (`CLE=valeur`) :

```text
RAM=512M
CPUS=2
DISK=build/vm/senbit.qcow2
DISK_SIZE=10G
CPU=host            # modèle de CPU QEMU : « host » expose les vraies instructions du CPU (requis par certains binaires)
DISK_FORMAT=qcow2
NETWORK=user
BOOT=bios           # bios ou uefi
```

* `--new` : supprime l'état de la VM et recrée le disque (tester l'installateur depuis zéro).
* `--debug` : journal série dans `build/vm/qemu.log`, diagnostics UEFI, journaux OVMF/QEMU, `qemu-cmdline.txt`.
* `--fast` : ne recompile que userspace/commandes/outils/rootfs/ISO.

Avec `BOOT=uefi`, le couple OVMF CODE/VARS est choisi de façon cohérente et la NVRAM
(`build/vm/OVMF_VARS.fd`) est vérifiée à chaque lancement (recréée si incompatible ; l'ancienne est
conservée en `OVMF_VARS.fd.stale`).

## Dépannage

* **Une étape se recompile alors que rien n'a changé** : supprimez `build/.state/cache.json` ou passez
  `--force` ; vérifiez qu'il n'y a pas de modification locale non commitée dans les sources.
* **Les sources de util-linux / parted / systemd / du noyau sont vides ou absentes** : lancez
  `devtool get` (`devtool build` le fait aussi tout seul). Les dossiers de sources ne doivent pas être
  commités comme dépôts git imbriqués : si `git submodule` se plaint de `external/systemd`, lancez
  `./scripts/git-unify.sh` (ou à la main : `git rm --cached external/systemd kernel/linux third_party/busybox third_party/util-linux third_party/parted`).
* **`rootfs` échoue sur les keymaps** : lancez d'abord `devtool build tools`.
* **`BOOT=uefi` : « no OVMF firmware found »** : installez `ovmf` (inclus dans `devtool install`).
* **QEMU affiche « Guest has not initialized the display (yet) » et `qemu.log` est vide** : le
  micrologiciel UEFI n'a pas démarré. Lancez `devtool run --debug` (couple de firmwares, état de la NVRAM,
  entrée EFI de l'ISO) ; `devtool run --new` réinitialise aussi la NVRAM. Si un shell UEFI apparaît,
  installez `grub-efi-amd64-bin` et `mtools` puis reconstruisez l'ISO.
* **Un programme dit « CPU ISA level is lower than required »** : gardez `CPU=host` dans `config/vm.config`.
* **Docker échoue sur `iptables` / nftables** : le noyau doit avoir été recompilé avec nftables IPv4/IPv6
  (`devtool build kernel`).
* **Il me faut toute la sortie d'une étape en échec** : `build/logs/`.
