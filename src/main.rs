use winreg::enums::*;
use winreg::RegKey;
use std::path::PathBuf;
use libpefile::*;
use lazy_static::lazy_static;
use regex::{Regex, Captures};
use serde::{Serialize, Deserialize};
use anyhow::Result;
use std::collections::HashMap;
use indicatif::ProgressBar;
use ron::ser::{to_string_pretty, PrettyConfig};

#[derive(Serialize, Deserialize)]
struct EventSources {
    filenames: HashMap<String, String>,
    sources: HashMap<String, EventSource>,
}

impl EventSources {
    pub fn new() -> Self {
        Self {
            filenames: HashMap::new(),
            sources: HashMap::new(),
        }
    }

    pub fn has_filename(&self, filename: &str) -> bool {
        self.sources.contains_key(filename)
    }

    pub fn add_source(&mut self, source: EventSource) -> Result<()> {
        if let Some(ref filename) = source.EventMessageFile {
            let filename = expand_env_vars(filename)?;
            self.filenames.insert(source.name.clone(), filename.clone());
            if ! self.has_filename(&filename) {
                self.sources.insert(filename, source);
            }
        }
        Ok(())
    }
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize)]
struct EventSource {
    pub name: String,
    pub CategoryCount: Option<u32>,
    pub CategoryMessageFile: Option<String>,
    pub EventMessageFile: Option<String>,
    pub ParameterMessageFile: Option<String>,
    pub TypesSupported: Option<u32>,
    pub messages: HashMap<u32, I18nMessages>,
}

impl EventSource {
    #[allow(non_snake_case)]
    pub fn from(name: String, key: &winreg::RegKey) -> std::io::Result<Self> {
        let CategoryCount = Self::get_value_u32(key, "CategoryCount");
        let CategoryMessageFile = Self::get_value_str(key, "CategoryMessageFile");
        let EventMessageFile = Self::get_value_str(key, "EventMessageFile");
        let ParameterMessageFile = Self::get_value_str(key, "ParameterMessageFile");
        let TypesSupported = Self::get_value_u32(key, "TypesSupported");
        Ok(Self {
            name,
            CategoryCount,
            CategoryMessageFile,
            EventMessageFile,
            ParameterMessageFile,
            TypesSupported,
            messages: HashMap::new(),
        })
    }

    pub fn add_message(&mut self, msg: Message) {
        if ! self.messages.contains_key(&msg.lang_id) {
            self.messages.insert(msg.lang_id, I18nMessages::new(msg.lang_id));
        }
        self.messages.get_mut(&msg.lang_id).unwrap().add_message(msg);
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

 
#[derive(Serialize, Deserialize)]
struct I18nMessages {
    pub lang_id: u32,
    pub messages: Vec<(u32, String)>,
}

impl I18nMessages {
    pub fn new(lang_id: u32) -> Self {
        Self {
            lang_id,
            messages: Vec::new()
        }
    }

    pub fn add_message(&mut self, msg: Message) {
        let mut text = msg.text;
        while text.ends_with("\n") || text.ends_with("\r") || text.ends_with("\u{0}") {
            text.pop();
        }
        self.messages.push((msg.msg_id, text));
    }
}

fn main() -> Result<()> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let eventlog_key = hklm.open_subkey("SYSTEM\\CurrentControlSet\\Services\\EventLog")?;
    let mut sources = EventSources::new();

    let app_count = count_keys(&eventlog_key)?;
    let bar = ProgressBar::new(app_count as u64);

    for app in eventlog_key.enum_keys().map(|x| x.unwrap()) {
        let appkey = eventlog_key.open_subkey(&app)?;
        sources.add_source(dump_key(&app, &appkey)?)?;
        bar.inc(1);

        for sub_app in appkey.enum_keys().map(|x| x.unwrap()) {
            let sub_app_name = format!("{}/{}", &app, &sub_app);
            let subkey = appkey.open_subkey(&sub_app)?;
            sources.add_source(dump_key(&sub_app_name, &subkey)?)?;
            bar.inc(1);
        }
    }

    
    let pretty = PrettyConfig::new()
        .with_depth_limit(2)
        .with_separate_tuple_members(true)
        .with_enumerate_arrays(false);
    let s = to_string_pretty(&sources, pretty).expect("Serialization failed");
    println!("{}", s);
    Ok(())
}

fn count_keys(eventlog_key: &winreg::RegKey) -> std::io::Result<u32> {
    let mut count = 0;

    for app in eventlog_key.enum_keys().map(|x| x.unwrap()) {
        let appkey = eventlog_key.open_subkey(&app)?;
        count += 1 + appkey.enum_keys().count();
    }
    Ok(count as u32)
}

fn dump_key(name: &str, key: &winreg::RegKey) -> std::io::Result<EventSource> {
    let mut event_source = EventSource::from(name.to_owned(), key)?;
    if let Some(ref event_message_file) = event_source.EventMessageFile {

        let expanded = expand_env_vars(&event_message_file)?;
        let file_names = expanded.split(';');
        for file_name in file_names {
            if let Ok(pefile) = PEFile::new(PathBuf::from(file_name)) {
                for msg in pefile.messages_iter()? {
                    match msg {
                        Err(why) => return Err(why),
                        Ok(msg) => event_source.add_message(msg)
                    }                    
                }
            };
        }
    }
    Ok(event_source)
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