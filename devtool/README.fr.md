# devtool

`devtool` est le remplaçant Rust de `scripts/build.sh`, `scripts/run.sh`,
`scripts/install.sh`, `scripts/getlinux.sh` et `scripts/getutil.sh`.

Il reprend exactement la même logique que ces scripts (mêmes chemins, mêmes
messages, même comportement), avec en plus :

* **Couleurs** : `==>` en vert pour les étapes, jaune pour "déjà à jour /
  sauté", rouge pour les erreurs.
* **Barre de progression / spinner** pendant les compilations longues
  (noyau, BusyBox, util-linux, Parted, userspace Rust, outils, ISO), avec
  la sortie de la commande sous-jacente coupée en cas de succès et
  affichée en entier seulement en cas d'échec — plus de mur de sortie
  `make`/`cargo`/`grub-mkrescue` à chaque build réussi.
* **Rebuild intelligent** : le noyau, BusyBox, util-linux, Parted et
  l'userspace Rust ne sont recompilés que si quelque chose a changé
  (nouveau commit récupéré, patch appliqué, modification locale, ou binaire
  de sortie manquant). L'état est gardé dans `build/.state/cache.json` —
  supprimez ce fichier, ou passez `--force`, pour forcer une recompilation
  complète.
* **Détection de changement de config BusyBox** : éditer
  `config/busybox.config` à la main est maintenant détecté automatiquement
  — `build/busybox/.config` et `build/busybox/_install` sont remis à zéro
  pour que la nouvelle config soit réellement prise en compte, au lieu de
  garder silencieusement l'ancienne.
* **`--jobs`/`-j`** : contrôle le nombre de jobs make/cargo en parallèle
  (par défaut, le nombre de CPU détecté, comme le `nproc` des anciens
  scripts).
* **`--force`** : vide le cache de rebuild avant de construire, pour tout
  recompiler même si rien ne semble avoir changé.
* **`build fast` / `run --fast`** : ne reconstruit que l'userspace Rust,
  les outils et le rootfs/ISO — saute entièrement les étapes
  noyau/BusyBox/util-linux/Parted (aucune vérification de version, aucune
  recompilation). Pensé pour itérer vite sur `senbit` (le binaire d'init,
  anciennement `senbit-init`) ou les outils, sans attendre le reste.
* **Support UEFI** : `config/vm.config` accepte une clé `BOOT` (`bios` par
  défaut, ou `uefi`) qui fait démarrer la VM avec un microgiciel OVMF au
  lieu du BIOS historique. L'ISO elle-même est déjà une image GRUB hybride
  BIOS+UEFI dès lors que `grub-efi-amd64-bin` est installé en plus de
  `grub-pc-bin` (voir `devtool install`).

## Compiler devtool

Prérequis : Rust (`rustc`/`cargo`), installables via `apt install rustc
cargo` ou [rustup](https://rustup.rs).

```
cd devtool
cargo build --release
```

Le binaire est produit dans `devtool/target/release/devtool`.

### Installation dans le projet Senbit

Placez le binaire compilé dans `scripts/`, à la place des anciens `.sh` :

```
cp devtool/target/release/devtool /chemin/vers/Senbit/scripts/devtool
cd /chemin/vers/Senbit
./scripts/devtool build
```

`devtool` détecte automatiquement la racine du projet (`ROOT_DIR`) à partir
de l'emplacement de l'exécutable, exactement comme le faisait
`$(dirname "${BASH_SOURCE[0]}")/..` dans les scripts bash — à condition
qu'il se trouve dans un dossier `scripts/`. Sinon :

* `devtool --root /chemin/vers/Senbit <commande>`
* ou la variable d'environnement `SENBIT_ROOT=/chemin/vers/Senbit`

## Utilisation

### `devtool build [cible] [-j/--jobs N] [--force]`

Équivalent de `scripts/build.sh [cible]`. Cibles disponibles :
`all` (défaut), `fast`, `kernel`, `busybox`, `util-linux`, `parted`,
`rust`, `tools`, `rootfs`, `iso`.

```
devtool build            # tout compiler (identique à ./scripts/build.sh)
devtool build kernel     # une seule étape
devtool build rootfs
devtool build --jobs 4   # limiter le parallélisme au lieu de tous les CPU
devtool build --force    # ignorer le cache, tout recompiler
devtool build fast       # rust + tools + rootfs + iso seulement
```

`all` construit dans l'ordre : noyau → BusyBox → util-linux → Parted →
userspace Rust → outils (`tools/`, y compris la génération des keymaps) →
rootfs/initramfs → ISO. `tools` et `rootfs` sont liés : le rootfs a besoin
des `.bmap` générés par les outils sous `build/tools/generated/keymaps/`.

`fast` est un ajout propre à devtool : il saute complètement les étapes
noyau/BusyBox/util-linux/Parted (aucune vérification de version, aucun
`make`), et ne reconstruit que `senbit` (le binaire d'init Rust), les
outils, et le rootfs/ISO qui les embarquent. Utile quand seul l'userspace
ou un outil a changé.

Différence notable avec l'ancien `getbusy.sh` : ce dernier était en réalité
une ancienne copie de `build.sh` (pas un script de récupération) — il n'a
pas de sous-commande dédiée, `devtool build` fait tout ce qu'il faisait, en
plus complet et à jour.

### `devtool get <linux|busybox|util-linux>`

Récupère ou met à jour les sources d'un composant, indépendamment d'une
compilation.

```
devtool get linux         # équivalent de scripts/getlinux.sh
                            # (clone/checkout la version de config/kernel/version)
devtool get util-linux     # équivalent de scripts/getutil.sh
                            # (clone/checkout la version de config/util-linux/version,
                            #  indépendant du sous-module utilisé par `devtool build util-linux`)
devtool get busybox        # pas d'ancien script dédié : reprend la logique de
                            # mise à jour vers la dernière version utilisée par
                            # `devtool build busybox`, sans lancer la compilation
```

Note sur le noyau : `devtool build kernel` **ne** récupère **pas**
automatiquement la dernière version — il compile simplement ce qui est déjà
cloné. Seul `devtool get linux` gère l'épinglage de version
(`config/kernel/version`). BusyBox reste différent : `devtool build
busybox` récupère toujours la dernière version disponible avant de
compiler.

### `devtool install`

Équivalent de `scripts/install.sh` : installe les paquets système
nécessaires (`apt-get`) puis récupère le noyau épinglé. Ne doit pas être
lancé en root. La liste de paquets inclut désormais `grub-efi-amd64-bin`
et `ovmf` (pour le support UEFI), et utilise le nom de paquet actuel
`uuid-dev` (l'ancien `libuuid-dev` n'existe plus sur les Debian/Ubuntu
récentes).

```
devtool install
```

### `devtool run [--new] [--debug] [--fast] [--force] [-j/--jobs N]`

Équivalent de `scripts/run.sh` : construit Senbit (`devtool build`, cible
`all` sauf si `--fast` est passé) puis le lance dans QEMU. Lit
`config/vm.config` (format `CLEF=valeur`) :

```
RAM=512M
CPUS=2
DISK=build/vm/senbit.qcow2
DISK_SIZE=10G
DISK_FORMAT=qcow2
NETWORK=user
BOOT=bios
```

```
devtool run             # build + lance la VM existante
devtool run --new       # supprime l'état de la VM et recrée le disque
devtool run --debug     # sortie série redirigée vers build/vm/qemu.log
devtool run --fast      # reconstruit juste userspace/tools/rootfs/iso, puis lance
```

`BOOT=uefi` démarre la VM via un microgiciel OVMF (détecté automatiquement
sous `/usr/share/OVMF` ou `/usr/share/ovmf`) au lieu du BIOS historique.
L'ISO n'a pas besoin d'être reconstruite pour changer de mode — c'est déjà
une image hybride, ce réglage change seulement la façon dont QEMU la
démarre.

## Résolution de problèmes

* **Une étape "compile" alors que rien n'a changé** : supprimez
  `build/.state/cache.json` (ou passez `--force`) pour repartir d'un cache
  propre, ou vérifiez qu'aucune modification locale non commitée ne traîne
  dans les sources concernées (`git status` dans `kernel/linux`,
  `third_party/busybox`, etc.).
* **J'ai édité `config/busybox.config` et rien n'a changé** : c'est
  maintenant géré automatiquement — devtool détecte le changement et remet
  à zéro `build/busybox/.config` et `build/busybox/_install` avant de
  reconstruire.
* **`util-linux`/`parted` refusent de compiler** : ce sont des sous-modules
  Git — `devtool build` attend qu'ils soient déjà sur le bon tag
  (`git submodule update --init --recursive`, puis `git checkout vX.Y.Z` si
  besoin). `devtool get util-linux` fournit un clonage direct indépendant
  si vous ne voulez pas utiliser de sous-module.
* **`rootfs` échoue sur les keymaps** : lancez `devtool build tools` (ou
  `devtool build all`) avant `devtool build rootfs` — les `.bmap` doivent
  exister dans `build/tools/generated/keymaps/`.
* **`BOOT=uefi` échoue avec "no OVMF firmware found"** : `sudo apt-get
  install ovmf` (déjà inclus dans `devtool install`).
* **Une étape échoue et j'ai besoin de la sortie complète du
  compilateur/outil** : elle est automatiquement affichée en entier en cas
  d'échec (seules les étapes réussies restent silencieuses).

[🇬🇧 English version](README.md)
