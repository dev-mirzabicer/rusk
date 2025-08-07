use serde::Deserialize;
use figment::{Figment, providers::{Format, Toml, Env}};

#[derive(Deserialize, Debug)]
pub struct Config {
    pub default_filters: Vec<String>,
}

impl Config {
    pub fn new() -> Result<Self, figment::Error> {
        Figment::new()
            .merge(Toml::file("config.toml"))
            .merge(Env::prefixed("TASK_"))
            .extract()
    }
}