//! Firmware UEFI (OVMF) pour `devtool run` avec `BOOT=uefi`.
//!
//! Pourquoi ce module existe : un couple CODE/VARS OVMF incohérent, ou une
//! NVRAM (`build/vm/OVMF_VARS.fd`) héritée d'un autre firmware, ne produit
//! AUCUNE erreur côté QEMU. Le firmware ne démarre simplement pas : le port
//! série reste muet (`qemu.log` vide) et la fenêtre affiche
//! « Guest has not initialized the display (yet) ». Ce symptôme est
//! indiscernable d'un problème de GRUB ou de noyau alors qu'il se produit
//! AVANT eux. Ici on :
//!
//! 1. choisit le firmware comme un COUPLE cohérent (descripteurs QEMU
//!    `firmware/*.json`, sinon même dossier + même variante) ;
//! 2. valide la NVRAM existante contre le modèle (taille + en-tête du volume
//!    de firmware) et la recrée si elle n'est pas compatible ;
//! 3. inspecte l'ISO (catalogue El Torito) pour savoir si elle est
//!    amorçable en UEFI, ce que « amorçable » tout court ne garantit pas.

use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::host;

/// D'où vient le couple de firmwares choisi (affiché en mode debug).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Descripteur QEMU (`/usr/share/qemu/firmware/*.json`).
    Descriptor(PathBuf),
    /// Couple trouvé dans un même dossier connu (`host::OVMF_DIRS`).
    Directory(PathBuf),
}

/// Couple cohérent CODE (lecture seule) + modèle de NVRAM.
#[derive(Debug, Clone)]
pub struct Firmware {
    pub code: PathBuf,
    pub vars_template: PathBuf,
    pub source: Source,
}

impl Firmware {
    pub fn describe_source(&self) -> String {
        match &self.source {
            Source::Descriptor(p) => format!("QEMU firmware descriptor {}", p.display()),
            Source::Directory(d) => format!("directory {}", d.display()),
        }
    }
}

// ---------------------------------------------------------------------------
// 1. Choix du firmware
// ---------------------------------------------------------------------------

/// Dossiers de descripteurs QEMU, du moins prioritaire au plus prioritaire
/// (un fichier du même nom dans un dossier suivant remplace le précédent).
fn descriptor_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/usr/share/qemu/firmware"),
        PathBuf::from("/usr/local/share/qemu/firmware"),
        PathBuf::from("/etc/qemu/firmware"),
    ];
    if let Some(cfg) = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    {
        dirs.push(cfg.join("qemu").join("firmware"));
    }
    dirs
}

#[derive(Debug, Deserialize)]
struct FileSpec {
    filename: String,
    #[serde(default)]
    format: String,
}

#[derive(Debug, Deserialize, Default)]
struct Mapping {
    #[serde(default)]
    device: String,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    executable: Option<FileSpec>,
    #[serde(default, rename = "nvram-template")]
    nvram_template: Option<FileSpec>,
}

#[derive(Debug, Deserialize)]
struct Target {
    #[serde(default)]
    architecture: String,
    #[serde(default)]
    machines: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Descriptor {
    #[serde(default, rename = "interface-types")]
    interface_types: Vec<String>,
    #[serde(default)]
    mapping: Mapping,
    #[serde(default)]
    targets: Vec<Target>,
    #[serde(default)]
    features: Vec<String>,
}

/// Le descripteur décrit-il un firmware utilisable tel quel par `devtool run` ?
/// QEMU est lancé SANS `-machine` (donc `pc`/i440fx), sans SMM ni Secure Boot :
/// on écarte les variantes qui les exigent.
fn descriptor_usable(d: &Descriptor) -> Option<(PathBuf, PathBuf)> {
    if !d.interface_types.iter().any(|t| t == "uefi") {
        return None;
    }
    // `mode` est facultatif : « split » (CODE + VARS séparés) par défaut.
    if d.mapping.device != "flash" || d.mapping.mode.as_deref().unwrap_or("split") != "split" {
        return None;
    }
    // Seules les fonctions qui IMPOSENT quelque chose à la machine sont
    // écartées. `amd-sev`, `acpi-s3`... indiquent juste ce que le firmware sait
    // faire (le descripteur Ubuntu « sans Secure Boot » annonce `amd-sev`).
    let blocked = ["secure-boot", "requires-smm", "enrolled-keys"];
    if d.features.iter().any(|f| blocked.contains(&f.as_str())) {
        return None;
    }
    let runs_on_i440fx = d.targets.iter().any(|t| {
        t.architecture == "x86_64" && t.machines.iter().any(|m| m.starts_with("pc-i440fx-"))
    });
    if !runs_on_i440fx {
        return None;
    }
    let code = d.mapping.executable.as_ref()?;
    let vars = d.mapping.nvram_template.as_ref()?;
    if code.format != "raw" || vars.format != "raw" {
        return None; // qcow2 : non géré par `-drive if=pflash,format=raw`
    }
    Some((PathBuf::from(&code.filename), PathBuf::from(&vars.filename)))
}

/// Premier descripteur utilisable parmi `dirs` (ordre alphabétique des noms,
/// les dossiers suivants masquant les précédents, comme le fait QEMU).
fn pick_from_descriptors(dirs: &[PathBuf]) -> Option<Firmware> {
    let mut by_name: BTreeMap<String, PathBuf> = BTreeMap::new();
    for dir in dirs {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    by_name.insert(name.to_string(), path);
                }
            }
        }
    }
    for path in by_name.values() {
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        let Ok(desc) = serde_json::from_str::<Descriptor>(&text) else {
            continue; // fichier vide / masquant : ignoré
        };
        if let Some((code, vars)) = descriptor_usable(&desc) {
            if code.is_file() && vars.is_file() {
                return Some(Firmware {
                    code,
                    vars_template: vars,
                    source: Source::Descriptor(path.clone()),
                });
            }
        }
    }
    None
}

/// Couples CODE/VARS connus, dans l'ordre de préférence. Un couple n'est
/// valable que si ses DEUX fichiers sont dans le MÊME dossier : mélanger
/// `OVMF_CODE_4M.fd` d'un dossier et `OVMF_VARS.fd` d'un autre (variantes
/// 4 Mo et 2 Mo) donne un firmware qui ne démarre pas.
const KNOWN_PAIRS: &[(&str, &str)] = &[
    ("OVMF_CODE_4M.fd", "OVMF_VARS_4M.fd"), // Debian / Ubuntu
    ("OVMF_CODE.4m.fd", "OVMF_VARS.4m.fd"), // Arch
    ("OVMF_CODE.fd", "OVMF_VARS.fd"),       // Fedora, anciens paquets
];

fn pick_from_directories(dirs: &[&str]) -> Option<Firmware> {
    for dir in dirs {
        let dir = Path::new(dir);
        for (code, vars) in KNOWN_PAIRS {
            let (c, v) = (dir.join(code), dir.join(vars));
            if c.is_file() && v.is_file() {
                return Some(Firmware {
                    code: c,
                    vars_template: v,
                    source: Source::Directory(dir.to_path_buf()),
                });
            }
        }
    }
    None
}

/// Trouve un couple de firmwares OVMF cohérent sur cette machine.
pub fn find_firmware() -> Result<Firmware> {
    if let Some(fw) = pick_from_descriptors(&descriptor_dirs()) {
        return Ok(fw);
    }
    if let Some(fw) = pick_from_directories(host::OVMF_DIRS) {
        return Ok(fw);
    }
    bail!(
        "BOOT=uefi requires the OVMF firmware (CODE and VARS of the SAME build), not found.\n\n\
         Install it:\n\
         \u{20} Ubuntu/Debian: sudo apt-get install ovmf\n\
         \u{20} Fedora:        sudo dnf install edk2-ovmf\n\
         \u{20} Arch:          sudo pacman -S edk2-ovmf\n\n\
         (or run `devtool install`), or set BOOT=bios in config/vm.config."
    )
}

// ---------------------------------------------------------------------------
// 2. Validation de la NVRAM
// ---------------------------------------------------------------------------

/// Résultat de `prepare_nvram`.
#[derive(Debug, PartialEq, Eq)]
pub enum Nvram {
    /// Aucune NVRAM : copiée depuis le modèle.
    Created,
    /// NVRAM existante, compatible avec le modèle : conservée.
    Reused,
    /// NVRAM incompatible : mise de côté (`.stale`) et recréée. Contient la raison.
    Refreshed(String),
}

/// Longueur de l'en-tête du volume de firmware EDK2 (`_FVH`), si valide.
fn fv_header_len(buf: &[u8]) -> Option<usize> {
    if buf.len() < 56 || &buf[40..44] != b"_FVH" {
        return None;
    }
    let len = u16::from_le_bytes([buf[48], buf[49]]) as usize;
    (56..=4096).contains(&len).then_some(len).filter(|l| *l <= buf.len())
}

/// `None` si la NVRAM peut être réutilisée avec ce modèle, sinon la raison.
fn nvram_incompatibility(nvram: &[u8], template: &[u8]) -> Option<String> {
    if nvram.len() != template.len() {
        return Some(format!(
            "size {} bytes differs from the firmware template ({} bytes): it belongs to another OVMF build",
            nvram.len(),
            template.len()
        ));
    }
    match fv_header_len(template) {
        Some(hl) => {
            if fv_header_len(nvram).is_none() {
                return Some("not a valid UEFI variable store (no firmware volume header)".into());
            }
            if nvram[..hl] != template[..hl] {
                return Some(
                    "firmware volume header differs from the template (different OVMF layout)"
                        .into(),
                );
            }
            None
        }
        // Modèle sans en-tête reconnaissable : seule la taille est vérifiable.
        None => None,
    }
}

/// S'assure que `vars` est une NVRAM utilisable avec `template`.
///
/// La NVRAM est un fichier PERSISTANT de la VM (`build/vm/OVMF_VARS.fd`) :
/// elle survit aux `devtool run` et, avant ce module, n'était jamais revérifiée.
/// Après une mise à jour du paquet OVMF (ou un changement de firmware), un
/// fichier hérité d'une autre variante empêche le firmware de démarrer.
pub fn prepare_nvram(vars: &Path, template: &Path) -> Result<Nvram> {
    let tmpl = fs::read(template)
        .with_context(|| format!("cannot read OVMF VARS template {}", template.display()))?;

    if !vars.is_file() {
        if let Some(parent) = vars.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(vars, &tmpl)?;
        return Ok(Nvram::Created);
    }

    let current = fs::read(vars)
        .with_context(|| format!("cannot read NVRAM {}", vars.display()))?;

    match nvram_incompatibility(&current, &tmpl) {
        None => Ok(Nvram::Reused),
        Some(reason) => {
            let mut stale = vars.as_os_str().to_owned();
            stale.push(".stale");
            let stale = PathBuf::from(stale);
            let _ = fs::remove_file(&stale);
            fs::rename(vars, &stale).with_context(|| {
                format!("cannot move incompatible NVRAM to {}", stale.display())
            })?;
            fs::write(vars, &tmpl)?;
            Ok(Nvram::Refreshed(reason))
        }
    }
}

// ---------------------------------------------------------------------------
// 3. ISO : amorçable en UEFI ?
// ---------------------------------------------------------------------------

/// Plateformes amorçables déclarées dans le catalogue El Torito de l'ISO.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct IsoBoot {
    /// Entrée BIOS (plateforme 80x86).
    pub bios: bool,
    /// Entrée EFI (plateforme 0xEF) : indispensable pour OVMF.
    pub efi: bool,
}

const SECTOR: u64 = 2048;

fn read_sector<R: Read + Seek>(r: &mut R, lba: u64) -> Result<[u8; SECTOR as usize]> {
    let mut buf = [0u8; SECTOR as usize];
    r.seek(SeekFrom::Start(lba * SECTOR))?;
    r.read_exact(&mut buf)?;
    Ok(buf)
}

/// Lit le catalogue El Torito d'une image ISO 9660.
pub fn inspect_iso_reader<R: Read + Seek>(r: &mut R) -> Result<IsoBoot> {
    // Descripteurs de volume : secteurs 16.. jusqu'au terminateur (type 255).
    let mut catalog_lba = None;
    for lba in 16..48u64 {
        let sec = read_sector(r, lba)?;
        if &sec[1..6] != b"CD001" {
            bail!("not an ISO 9660 image (no CD001 descriptor at sector {lba})");
        }
        match sec[0] {
            0 if sec[7..30].starts_with(b"EL TORITO SPECIFICATION") => {
                catalog_lba = Some(u32::from_le_bytes([sec[71], sec[72], sec[73], sec[74]]) as u64);
                break;
            }
            255 => break,
            _ => {}
        }
    }
    let Some(catalog_lba) = catalog_lba else {
        return Ok(IsoBoot::default()); // pas de boot record : non amorçable du tout
    };

    let cat = read_sector(r, catalog_lba)?;
    let mut out = IsoBoot::default();

    let mark = |platform: u8, bootable: bool, out: &mut IsoBoot| {
        if bootable {
            match platform {
                0x00 => out.bios = true,
                0xEF => out.efi = true,
                _ => {}
            }
        }
    };

    // Entrée de validation (32 o) : en-tête 1, plateforme, clé 0x55AA.
    if cat[0] != 1 || cat[30] != 0x55 || cat[31] != 0xAA {
        bail!("invalid El Torito boot catalog (bad validation entry)");
    }
    let default_platform = cat[1];
    // Entrée initiale/par défaut (32 o) : 0x88 = amorçable.
    mark(default_platform, cat[32] == 0x88, &mut out);

    // Sections suivantes : en-tête 0x90/0x91, plateforme, nombre d'entrées.
    let mut off = 64;
    while off + 32 <= cat.len() && (cat[off] == 0x90 || cat[off] == 0x91) {
        let last = cat[off] == 0x91;
        let platform = cat[off + 1];
        let count = u16::from_le_bytes([cat[off + 2], cat[off + 3]]) as usize;
        off += 32;
        for _ in 0..count {
            if off + 32 > cat.len() {
                break;
            }
            mark(platform, cat[off] == 0x88, &mut out);
            let mut more = cat[off + 1] & 0x20 != 0; // entrées d'extension (0x44)
            off += 32;
            while more && off + 32 <= cat.len() && cat[off] == 0x44 {
                more = cat[off + 1] & 0x20 != 0;
                off += 32;
            }
        }
        if last {
            break;
        }
    }
    Ok(out)
}

pub fn inspect_iso(path: &Path) -> Result<IsoBoot> {
    let mut f = fs::File::open(path)
        .with_context(|| format!("cannot open ISO {}", path.display()))?;
    inspect_iso_reader(&mut f)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Faux volume de firmware EDK2 : en-tête de 72 octets + remplissage.
    fn fake_fv(len: usize, tag: u8) -> Vec<u8> {
        let mut v = vec![0xFFu8; len];
        v[..16].fill(0);
        v[16..32].fill(tag); // « GUID » du système de fichiers
        v[32..40].copy_from_slice(&(len as u64).to_le_bytes());
        v[40..44].copy_from_slice(b"_FVH");
        v[48..50].copy_from_slice(&72u16.to_le_bytes());
        v
    }

    fn tmpdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("senbit-ovmf-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn nvram_created_then_reused() {
        let d = tmpdir("created");
        let (t, v) = (d.join("t.fd"), d.join("v.fd"));
        fs::write(&t, fake_fv(4096, 1)).unwrap();
        assert_eq!(prepare_nvram(&v, &t).unwrap(), Nvram::Created);
        assert_eq!(prepare_nvram(&v, &t).unwrap(), Nvram::Reused);
    }

    #[test]
    fn nvram_keeps_guest_variables() {
        let d = tmpdir("keeps");
        let (t, v) = (d.join("t.fd"), d.join("v.fd"));
        fs::write(&t, fake_fv(4096, 1)).unwrap();
        let mut used = fake_fv(4096, 1);
        used[2000] = 0x42; // variable écrite par l'invité
        fs::write(&v, &used).unwrap();
        assert_eq!(prepare_nvram(&v, &t).unwrap(), Nvram::Reused);
        assert_eq!(fs::read(&v).unwrap()[2000], 0x42);
    }

    #[test]
    fn nvram_of_another_size_is_replaced() {
        let d = tmpdir("size");
        let (t, v) = (d.join("t.fd"), d.join("v.fd"));
        fs::write(&t, fake_fv(8192, 1)).unwrap();
        fs::write(&v, fake_fv(4096, 1)).unwrap(); // ancienne variante « 2M »
        match prepare_nvram(&v, &t).unwrap() {
            Nvram::Refreshed(r) => assert!(r.contains("size"), "{r}"),
            other => panic!("expected Refreshed, got {other:?}"),
        }
        assert_eq!(fs::read(&v).unwrap().len(), 8192);
        assert!(d.join("v.fd.stale").is_file(), "old NVRAM must be kept aside");
    }

    #[test]
    fn nvram_garbage_or_other_layout_is_replaced() {
        let d = tmpdir("garbage");
        let (t, v) = (d.join("t.fd"), d.join("v.fd"));
        fs::write(&t, fake_fv(4096, 1)).unwrap();

        fs::write(&v, vec![0xA5u8; 4096]).unwrap(); // même taille, pas un FV
        assert!(matches!(prepare_nvram(&v, &t).unwrap(), Nvram::Refreshed(_)));

        fs::write(&v, fake_fv(4096, 2)).unwrap(); // FV valide d'une autre variante
        assert!(matches!(prepare_nvram(&v, &t).unwrap(), Nvram::Refreshed(_)));
        assert_eq!(fs::read(&v).unwrap(), fake_fv(4096, 1));
    }

    #[test]
    fn directory_pairs_never_mix_variants() {
        let d = tmpdir("pairs");
        // CODE 4M seul dans un dossier, VARS 2M seul dans un autre :
        let a = d.join("a");
        let b = d.join("b");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        fs::write(a.join("OVMF_CODE_4M.fd"), b"c").unwrap();
        fs::write(b.join("OVMF_VARS.fd"), b"v").unwrap();
        let dirs = [a.to_str().unwrap(), b.to_str().unwrap()];
        assert!(pick_from_directories(&dirs).is_none());

        fs::write(a.join("OVMF_VARS_4M.fd"), b"v").unwrap();
        let fw = pick_from_directories(&dirs).unwrap();
        assert!(fw.code.ends_with("OVMF_CODE_4M.fd"));
        assert!(fw.vars_template.ends_with("OVMF_VARS_4M.fd"));
        assert_eq!(fw.vars_template.parent(), fw.code.parent());
    }

    fn write_descriptor(dir: &Path, name: &str, code: &Path, vars: &Path, features: &str) {
        // Comme le descripteur Ubuntu réel : pas de clé `mode`, feature `amd-sev` annoncée.
        let json = format!(
            r#"{{"interface-types":["uefi"],
                "mapping":{{"device":"flash",
                  "executable":{{"filename":"{}","format":"raw"}},
                  "nvram-template":{{"filename":"{}","format":"raw"}}}},
                "targets":[{{"architecture":"x86_64","machines":["pc-i440fx-*","pc-q35-*"]}}],
                "features":[{features}]}}"#,
            code.display(),
            vars.display()
        );
        fs::write(dir.join(name), json).unwrap();
    }

    #[test]
    fn descriptors_skip_secure_boot_and_pick_a_pair() {
        let d = tmpdir("desc");
        let fw = d.join("fw");
        fs::create_dir_all(&fw).unwrap();
        for f in ["c_sb.fd", "v_sb.fd", "c.fd", "v.fd"] {
            fs::write(d.join(f), b"x").unwrap();
        }
        write_descriptor(&fw, "40-sb.json", &d.join("c_sb.fd"), &d.join("v_sb.fd"),
            r#""secure-boot","requires-smm""#);
        write_descriptor(&fw, "60-plain.json", &d.join("c.fd"), &d.join("v.fd"), r#""acpi-s3","amd-sev""#);
        let got = pick_from_descriptors(&[fw.clone()]).unwrap();
        assert!(got.code.ends_with("c.fd"));
        assert!(got.vars_template.ends_with("v.fd"));
        assert_eq!(got.source, Source::Descriptor(fw.join("60-plain.json")));
    }

    #[test]
    fn descriptors_with_missing_files_are_ignored() {
        let d = tmpdir("desc-missing");
        write_descriptor(&d, "60.json", &d.join("nope_c.fd"), &d.join("nope_v.fd"), "");
        assert!(pick_from_descriptors(&[d]).is_none());
    }

    /// ISO minimale : boot record au secteur 17, catalogue au secteur 20.
    fn fake_iso(bios: bool, efi: bool) -> Vec<u8> {
        let mut iso = vec![0u8; 24 * SECTOR as usize];
        let pvd = 16 * SECTOR as usize;
        iso[pvd] = 1;
        iso[pvd + 1..pvd + 6].copy_from_slice(b"CD001");
        let br = 17 * SECTOR as usize;
        iso[br] = 0;
        iso[br + 1..br + 6].copy_from_slice(b"CD001");
        iso[br + 6] = 1;
        iso[br + 7..br + 30].copy_from_slice(b"EL TORITO SPECIFICATION");
        iso[br + 71..br + 75].copy_from_slice(&20u32.to_le_bytes());
        let term = 18 * SECTOR as usize;
        iso[term] = 255;
        iso[term + 1..term + 6].copy_from_slice(b"CD001");

        let cat = 20 * SECTOR as usize;
        iso[cat] = 1; // validation
        iso[cat + 1] = 0; // plateforme par défaut : BIOS
        iso[cat + 30] = 0x55;
        iso[cat + 31] = 0xAA;
        iso[cat + 32] = if bios { 0x88 } else { 0x00 };
        if efi {
            iso[cat + 64] = 0x91; // dernier en-tête de section
            iso[cat + 65] = 0xEF;
            iso[cat + 66] = 1; // une entrée
            iso[cat + 96] = 0x88;
        }
        iso
    }

    #[test]
    fn iso_hybrid_has_both_entries() {
        let b = inspect_iso_reader(&mut Cursor::new(fake_iso(true, true))).unwrap();
        assert_eq!(b, IsoBoot { bios: true, efi: true });
    }

    #[test]
    fn iso_bios_only_is_detected() {
        let b = inspect_iso_reader(&mut Cursor::new(fake_iso(true, false))).unwrap();
        assert_eq!(b, IsoBoot { bios: true, efi: false });
    }

    #[test]
    fn iso_without_boot_record_or_not_an_iso() {
        let mut iso = fake_iso(true, true);
        iso[17 * SECTOR as usize + 7] = b'X'; // casse la signature El Torito
        assert_eq!(inspect_iso_reader(&mut Cursor::new(iso)).unwrap(), IsoBoot::default());
        assert!(inspect_iso_reader(&mut Cursor::new(vec![0u8; 64 * 2048])).is_err());
    }

    /// Test d'intégration (QEMU + OVMF requis) :
    /// `cargo test --manifest-path devtool/Cargo.toml -- --ignored boots_ovmf`.
    /// Démarre le firmware (avec une NVRAM invalide réparée par
    /// `prepare_nvram`) sans disque ni ISO et vérifie qu'il atteint le gestionnaire
    /// de démarrage : c'est ce que `qemu.log` doit contenir quand le firmware
    /// démarre, et ce qui manque (ou est remplacé par un dump d'exception)
    /// quand il ne démarre pas.
    #[test]
    #[ignore]
    fn boots_ovmf_headless_and_writes_to_serial() {
        let fw = find_firmware().expect("OVMF installed");
        let d = tmpdir("boot");
        let vars = d.join("OVMF_VARS.fd");
        // NVRAM volontairement invalide, de la bonne taille (le cas qui fait
        // silencieusement échouer OVMF) : doit être réparée avant le démarrage.
        let len = fs::metadata(&fw.vars_template).unwrap().len() as usize;
        fs::write(&vars, vec![0xA5u8; len]).unwrap();
        assert!(matches!(prepare_nvram(&vars, &fw.vars_template).unwrap(), Nvram::Refreshed(_)));

        let log = d.join("serial.log");
        let mut child = host::command("qemu-system-x86_64")
            .args(["-m", "256M", "-display", "none", "-monitor", "none", "-nic", "user"])
            .args(["-drive", &format!("if=pflash,format=raw,readonly=on,file={}", fw.code.display())])
            .args(["-drive", &format!("if=pflash,format=raw,file={}", vars.display())])
            .args(["-serial", &format!("file:{}", log.display())])
            .spawn()
            .expect("qemu-system-x86_64");

        // Sans média amorçable, un OVMF sain finit par tenter le démarrage
        // réseau (« Start PXE ») ; un OVMF cassé par une NVRAM invalide soit
        // reste muet, soit écrit un dump d'exception CPU (« Exception Type »).
        let mut text = String::new();
        for _ in 0..120 {
            std::thread::sleep(std::time::Duration::from_secs(1));
            text = fs::read(&log)
                .map(|b| String::from_utf8_lossy(&b).into_owned())
                .unwrap_or_default();
            if text.contains("Start PXE") || text.contains("Exception Type") {
                break;
            }
        }
        let _ = child.kill();
        let _ = child.wait();

        assert!(!text.contains("Exception Type"), "OVMF crashed:\n{text}");
        assert!(
            text.contains("Start PXE"),
            "OVMF never reached the boot manager (serial log: {} bytes): the firmware did not start",
            text.len()
        );
    }
}
