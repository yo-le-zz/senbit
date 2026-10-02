// installation/local/local.rs

use crate::log_info;
use anyhow::Result;
use colored::Colorize;
use inquire::Select;
use std::fs::File;
use std::io::Write;
use std::path::Path;

const LANGUAGES_LIST: &[&str] = &[
    "en_US.UTF-8",
    "en_GB.UTF-8",
    "fr_FR.UTF-8",
    "de_DE.UTF-8",
    "es_ES.UTF-8",
    "it_IT.UTF-8",
    "pt_PT.UTF-8",
    "pt_BR.UTF-8",
    "nl_NL.UTF-8",
    "pl_PL.UTF-8",
    "cs_CZ.UTF-8",
    "sk_SK.UTF-8",
    "hu_HU.UTF-8",
    "ro_RO.UTF-8",
    "bg_BG.UTF-8",
    "uk_UA.UTF-8",
    "ru_RU.UTF-8",
    "tr_TR.UTF-8",
    "el_GR.UTF-8",
    "da_DK.UTF-8",
    "sv_SE.UTF-8",
    "nb_NO.UTF-8",
    "fi_FI.UTF-8",
    "et_EE.UTF-8",
    "lv_LV.UTF-8",
    "lt_LT.UTF-8",
    "sl_SI.UTF-8",
    "hr_HR.UTF-8",
    "sr_RS.UTF-8",
    "he_IL.UTF-8",
    "ar_SA.UTF-8",
    "fa_IR.UTF-8",
    "hi_IN.UTF-8",
    "bn_BD.UTF-8",
    "th_TH.UTF-8",
    "vi_VN.UTF-8",
    "id_ID.UTF-8",
    "ms_MY.UTF-8",
    "ja_JP.UTF-8",
    "ko_KR.UTF-8",
    "zh_CN.UTF-8",
    "zh_TW.UTF-8",
];

pub fn setup_locale(mount_point: &Path) -> Result<()> {
    log_info!(
        "{}",
        "Setting up locale...".green()
    );

    let language = Select::new(
        "Select a language:",
        LANGUAGES_LIST.to_vec(),
    )
    .prompt()?;

    log_info!(
        "{}",
        format!("Language selected: {}", language).green()
    );

    write_locale_file(&language, mount_point)?;

    Ok(())
}

fn write_locale_file(
    language: &str,
    mount_point: &Path,
) -> Result<()> {
    let locale_path = mount_point.join("etc/locale.conf");

    let mut file = File::create(locale_path)?;

    writeln!(file, "LANG={}", language)?;

    Ok(())
}