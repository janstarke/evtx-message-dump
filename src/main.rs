use winreg::enums::*;
use winreg::RegKey;
use std::path::PathBuf;
use libpefile::*;
use lazy_static::lazy_static;
use regex::{Regex, Captures};

#[allow(non_snake_case)]
struct EventSource {
    pub CategoryCount: Option<u32>,
    pub CategoryMessageFile: Option<String>,
    pub EventMessageFile: Option<String>,
    pub ParameterMessageFile: Option<String>,
    pub TypesSupported: Option<u32>
}

impl EventSource {
    #[allow(non_snake_case)]
    pub fn from(key: &winreg::RegKey) -> std::io::Result<Self> {
        let CategoryCount = Self::get_value_u32(key, "CategoryCount");
        let CategoryMessageFile = Self::get_value_str(key, "CategoryMessageFile");
        let EventMessageFile = Self::get_value_str(key, "EventMessageFile");
        let ParameterMessageFile = Self::get_value_str(key, "ParameterMessageFile");
        let TypesSupported = Self::get_value_u32(key, "TypesSupported");
        Ok(Self {
            CategoryCount,
            CategoryMessageFile,
            EventMessageFile,
            ParameterMessageFile,
            TypesSupported
        })
    }

    fn get_value_u32(key: &winreg::RegKey, name: &str) -> Option<u32> {
        match key.get_value::<u32, &str>(name) {
            Ok(v) => Some(v),
            _ => None
        }
    }
    fn get_value_str(key: &winreg::RegKey, name: &str) -> Option<String> {
        match key.get_value::<String, &str>(name) {
            Ok(v) => Some(v),
            _ => None
        }
    }
 }

fn main() -> std::io::Result<()> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let eventlog_key = hklm.open_subkey("SYSTEM\\CurrentControlSet\\Services\\EventLog")?;

    for app in eventlog_key.enum_keys().map(|x| x.unwrap()) {
        let appkey = eventlog_key.open_subkey(&app)?;
        dump_key(&app, &appkey)?;

        for sub_app in appkey.enum_keys().map(|x| x.unwrap()) {
            let sub_app_name = format!("{}/{}", &app, &sub_app);
            let subkey = appkey.open_subkey(&sub_app)?;
            dump_key(&sub_app_name, &subkey)?;
        }
    }
    Ok(())
}

fn dump_key(name: &str, key: &winreg::RegKey) -> std::io::Result<()> {
    let event_source = EventSource::from(key)?;
    if let Some(event_message_file) = event_source.EventMessageFile {
        println!("{}", name);

        let expanded = expand_env_vars(&event_message_file)?;
        let file_names = expanded.split(';');
        for file_name in file_names {
            println!("  {}", file_name);
            if let Ok(pefile) = PEFile::new(PathBuf::from(file_name)) {
                ()
            };
        }
    }
    Ok(())
}
/*
    From: https://users.rust-lang.org/t/expand-win-env-var-in-string/50320/3
*/
pub fn expand_env_vars(s:&str) -> std::io::Result<String>  {
    lazy_static! {
        static ref ENV_VAR: Regex = Regex::new("%([[:word:]]*)%").expect("Invalid Regex");
    }

    let result: String = ENV_VAR.replace_all(s, |c:&Captures| match &c[1] {
        "" => String::from("%"),
        varname => std::env::var(varname).expect("Bad Var Name")
    }).into();

    Ok(result)
}