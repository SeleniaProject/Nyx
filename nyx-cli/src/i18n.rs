//! i18n tables placeholder

#![forbid(unsafe_code)]

use std::collections::HashMap;

pub type I18nTable = HashMap<&'static str, &'static str>;

pub fn get_table(_lang: &str) -> I18nTable { HashMap::new() }

