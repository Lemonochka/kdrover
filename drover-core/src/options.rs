use std::fs;

use std::path::Path;



use ini::Ini;



use crate::udp_bypass::UdpBypassMode;



pub const BUILD_DLL_FILENAME: &str = "kdrover_payload.dll";

/// Filename used inside Discord's app folder after install.
pub const DLL_FILENAME: &str = "version.dll";

pub const OPTIONS_FILENAME: &str = "drover.ini";

pub const PACKET_FILENAME: &str = "drover-packet.bin";



#[derive(Debug, Clone)]

pub struct DroverOptions {

    pub proxy: String,

    pub udp_bypass: UdpBypassMode,

}



impl Default for DroverOptions {

    fn default() -> Self {

        Self {

            proxy: String::new(),

            udp_bypass: UdpBypassMode::Auto,

        }

    }

}



pub fn load_options(path: impl AsRef<Path>) -> DroverOptions {

    let path = path.as_ref();

    if !path.exists() {

        return DroverOptions::default();

    }



    match Ini::load_from_file(path) {

        Ok(ini) => {

            let section = ini.section(Some("drover"));

            DroverOptions {

                proxy: section

                    .and_then(|s| s.get("proxy"))

                    .unwrap_or("")

                    .trim()

                    .to_string(),

                udp_bypass: section

                    .and_then(|s| s.get("udp-bypass"))

                    .map(UdpBypassMode::parse)

                    .unwrap_or_default(),

            }

        }

        Err(_) => DroverOptions::default(),

    }

}



pub fn save_options(path: impl AsRef<Path>, options: &DroverOptions) -> std::io::Result<()> {

    let mut ini = Ini::new();

    ini.with_section(Some("drover"))

        .set("proxy", options.proxy.trim())

        .set("udp-bypass", options.udp_bypass.as_ini_value());

    ini.write_to_file(path.as_ref())

}



pub fn extra_filenames() -> [&'static str; 1] {

    [PACKET_FILENAME]

}



pub fn copy_drover_files(source_dir: &Path, target_dir: &Path) -> std::io::Result<()> {

    for name in [OPTIONS_FILENAME, DLL_FILENAME]

        .into_iter()

        .chain(extra_filenames().into_iter())

    {

        let source = source_dir.join(name);

        if source.exists() {

            fs::copy(&source, target_dir.join(name))?;

        }

    }

    Ok(())

}



pub fn remove_drover_files(dir: &Path) -> std::io::Result<()> {

    for name in [OPTIONS_FILENAME, DLL_FILENAME]

        .into_iter()

        .chain(extra_filenames().into_iter())

    {

        let path = dir.join(name);

        if path.exists() {

            fs::remove_file(path)?;

        }

    }

    Ok(())

}


