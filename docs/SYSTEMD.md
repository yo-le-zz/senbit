# Architecture de démarrage de Senbit (systemd)

```
Kernel
  └─ /init  (senbit-init, depuis l'initramfs)
        ├─ live (pas d'installation trouvée) → installeur
        └─ installé:
              monte la racine, vérifie fstab, hostname, réseau (busybox)
              switch_root (+ libère la RAM de l'initramfs)
              charge le clavier
              exec(/usr/lib/systemd/systemd --system)
                    └─ PID 1 = systemd
                          ├─ multi-user.target
                          │     └─ senbit-login.service → /sbin/senbit-login
                          └─ (futurs services : apt, réseau, ssh…)
```

## Qui fait quoi

| Composant | Rôle | Où |
|---|---|---|
| `senbit-init` | transition uniquement : monter, préparer, `exec(systemd)`. Contient aussi l'installeur. | `src/` |
| `senbit-login` | console de login + socket d'événements `/run/senbit.sock`. Crate indépendante. | `components/senbit-login/` |
| `senbit-login.service` | lance le login sur `/dev/console`, le relance s'il s'arrête | `rootfs/usr/lib/systemd/system/` |

`senbit-init` ne contient plus : login, shell, socket d'événements, `systemctl`,
`init_vars`, lancement de systemd en `spawn()`.

## Environnement des sessions

systemd démarre les services avec un environnement quasi vide (il n'hérite pas
de celui de l'init). `senbit-login` reconstruit donc l'environnement du shell
à chaque connexion :

| Variable | Source |
|---|---|
| `HOSTNAME` | `/etc/hostname` |
| `LANG`, `LC_ALL` | `/etc/default/locale` |
| `TZ` | `/etc/timezone` |
| `SENBIT_VERSION` | `/etc/senbit-release` |
| `USER`, `HOME`, `SHELL`… | `/etc/passwd` |

## Arrêt / redémarrage

`reboot`, `shutdown`, `halt`, `poweroff` écrivent un mot dans `/run/senbit.sock`
(protocole inchangé). `senbit-login` répond `ok` puis exécute
`systemctl --no-block <verbe>`. systemd arrête alors **tous** les services dans
l'ordre inverse des dépendances (ceux de Senbit et ceux que des paquets
ajouteront), puis démonte et éteint. Aucun nom de service n'est codé en dur.

## Compatibilité apt / paquets

* Les unités de paquets sont vues : `/usr/lib/systemd/system`,
  `/etc/systemd/system`, `/usr/local/lib/systemd/system`, et
  `/lib/systemd` → `/usr/lib/systemd` (les .deb installent dans `/lib/systemd/system`).
* `systemctl enable/disable/daemon-reload` et les scripts de paquets fonctionnent
  puisque systemd est PID 1 (`/run/systemd/system` existe).
* Le login ne propose que root et les utilisateurs normaux (UID ≥ 1000) avec un
  vrai shell et un mot de passe valide : les comptes système créés par les
  paquets (`www-data`, `_apt`…) n'apparaissent jamais.
* `/etc/machine-id` est créé vide à l'installation (premier démarrage).
* `/etc/os-release` existe (`ID=senbit`).

## Vérifications à faire dans la VM (BIOS et UEFI)

```sh
cat /proc/1/comm                  # → systemd
systemctl is-system-running       # → running (ou degraded, voir plus bas)
systemctl status senbit-login     # → active (running)
journalctl -u senbit-login -b     # logs du login
systemctl --failed                # unités en échec éventuelles
cat /etc/passwd                   # root une seule fois
```

`degraded` signifie qu'au moins une unité standard de systemd a échoué (pas de
udev/hwdb/blkid dans ce rootfs minimal) : `systemctl --failed` dit lesquelles ;
à masquer dans `devtool/src/build/systemd.rs` (`MASKED_UNITS`) si elles ne
servent pas.

## Build

`devtool build` compile maintenant les deux crates (`senbit` et `senbit-login`,
cibles musl statiques), installe `/sbin/senbit-login`, puis
`verify_rootfs()` contrôle le rootfs final : fichiers requis par systemd,
interpréteur ELF, `ExecStart` des unités `senbit-*`, liens d'activation.
Le noyau force (et vérifie) les options requises par systemd et la console UEFI.

## Debug : capturer toute la console

`devtool run --debug` écrit **tout le démarrage** dans `build/vm/qemu.log`
(noyau, senbit-init, systemd, journal, senbit-login) :

- devtool lance QEMU avec `-serial file:build/vm/qemu.log` et
  `-fw_cfg name=opt/senbit/debug,string=1` (le noyau expose ce drapeau via
  `CONFIG_FW_CFG_SYSFS`, épinglé dans `devtool/src/build/kernel.rs`) ;
- en mode debug, `senbit-init` recopie chaque ligne de log dans `/dev/kmsg`
  (`src/system/log/debug.rs`), lance systemd avec `--log-target=kmsg` et
  active `ForwardToKMsg` pour le journal ; un processus d'arrière-plan
  (`cat /dev/kmsg > /dev/ttyS0`) recopie tout le journal du noyau sur le port
  série : rien de plus n'est affiché à l'écran, les menus restent propres ;
- sans QEMU : ajouter `senbit.debug` à la ligne `linux` de GRUB.

Le mode debug fonctionne aussi bien pendant l'installation que sur le système
installé, tant que la VM est lancée avec `--debug`.
