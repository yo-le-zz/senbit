# devtool

`devtool` est le remplaçant Rust de `scripts/build.sh`, `scripts/run.sh`,
`scripts/install.sh`, `scripts/getlinux.sh` et `scripts/getutil.sh`.

Il reprend exactement la même logique que ces scripts (mêmes chemins, mêmes
messages, même comportement), avec en plus :

- **Couleurs** : `==>` en vert pour les étapes, jaune pour "déjà à jour /
  sauté", rouge pour les erreurs.
- **Barre de progression / spinner** pendant les compilations longues
  (noyau, BusyBox, util-linux, Parted, userspace Rust, outils, ISO).
- **Rebuild intelligent** : le noyau, BusyBox, util-linux, Parted et
  l'userspace Rust ne sont recompilés que si quelque chose a changé
  (nouveau commit récupéré, patch appliqué, modification locale, ou binaire
  de sortie manquant). L'état est gardé dans `build/.state/cache.json` —
  supprimez ce fichier pour forcer une recompilation complète.

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
cp devtool/target/release/devtool /chemin/vers/Senbit/scripts/devtool_bin/devtool
cd /chemin/vers/Senbit
./scripts/devtool_bin/devtool build
```

`devtool` détecte automatiquement la racine du projet (`ROOT_DIR`) à partir
de l'emplacement de l'exécutable, exactement comme le faisait
`$(dirname "${BASH_SOURCE[0]}")/..` dans les scripts bash — à condition
qu'il se trouve dans un dossier `scripts/`. Sinon :

- `devtool --root /chemin/vers/Senbit <commande>`
- ou la variable d'environnement `SENBIT_ROOT=/chemin/vers/Senbit`

## Utilisation

### `devtool build [cible]`

Équivalent de `scripts/build.sh [cible]`. Cibles disponibles :
`all` (défaut), `kernel`, `busybox`, `util-linux`, `parted`, `rust`,
`tools`, `rootfs`, `iso`.

```
devtool build            # tout compiler (identique à ./scripts/build.sh)
devtool build kernel     # une seule étape
devtool build rootfs
```

`all` construit dans l'ordre : noyau → BusyBox → util-linux → Parted →
userspace Rust → outils (`tools/`, y compris la génération des keymaps) →
rootfs/initramfs → ISO. `tools` et `rootfs` sont liés : le rootfs a besoin
des `.bmap` générés par les outils sous `build/tools/generated/keymaps/`.

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

Note sur le noyau : depuis la dernière version de `build.sh`,
`devtool build kernel` **ne** récupère **plus** automatiquement la dernière
version — il compile simplement ce qui est déjà cloné. Seul `devtool get
linux` gère l'épinglage de version (`config/kernel/version`). BusyBox reste
différent : `devtool build busybox` récupère toujours la dernière version
disponible avant de compiler.

### `devtool install`

Équivalent de `scripts/install.sh` : installe les paquets système
nécessaires (`apt-get`) puis récupère le noyau épinglé. Ne doit pas être
lancé en root.

```
devtool install
```

### `devtool run [--new] [--debug]`

Équivalent de `scripts/run.sh` : construit Senbit (`devtool build`, cible
`all`) puis le lance dans QEMU. Lit `config/vm.config` (format `CLEF=valeur`,
inchangé) :

```
RAM=512M
CPUS=2
DISK=build/vm/senbit.qcow2
DISK_SIZE=10G
DISK_FORMAT=qcow2
NETWORK=user
```

```
devtool run             # build + lance la VM existante
devtool run --new       # supprime l'état de la VM et recrée le disque
devtool run --debug     # sortie série redirigée vers build/vm/qemu.log
```

## Résolution de problèmes

- **Une étape "compile" alors que rien n'a changé** : supprimez
  `build/.state/cache.json` pour repartir d'un cache propre, ou vérifiez
  qu'aucune modification locale non commitée ne traîne dans les sources
  concernées (`git status` dans `kernel/linux`, `third_party/busybox`, etc.).
- **`util-linux`/`parted` refusent de compiler** : ce sont des sous-modules
  Git — `devtool build` attend qu'ils soient déjà sur le bon tag
  (`git submodule update --init --recursive`, puis `git checkout vX.Y.Z` si
  besoin). `devtool get util-linux` fournit un clonage direct indépendant
  si vous ne voulez pas utiliser de sous-module.
- **`rootfs` échoue sur les keymaps** : lancez `devtool build tools` (ou
  `devtool build all`) avant `devtool build rootfs` — les `.bmap` doivent
  exister dans `build/tools/generated/keymaps/`.

[🇬🇧 English version](README.md)