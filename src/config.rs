use clap::{arg, Parser, ValueEnum};
use clap_verbosity_flag::{InfoLevel, Verbosity};
use env_logger::TimestampPrecision;
use indicatif_log_bridge::LogWrapper;
use indicatif::MultiProgress;
use log::info;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use dotenv::dotenv;
use std::{
    fmt::{self, Display},
    fs::File,
    io::{BufReader, Write},
    path::PathBuf, sync::{Arc, Mutex},
};

#[derive(Debug, Clone, Parser, Default)]
pub struct Config {
    /// Your `FANBOXSESSID` cookie
    #[clap(env="FANBOXSESSID")]
    session: String,
    /// Which you path want to save
    #[arg(short, long, default_value = "./fanbox")]
    output: PathBuf,
    /// Which you type want to save
    #[arg(short, long, default_value = "supporting")]
    save: SaveType,
    /// Cache directory [default: "."]
    #[arg(short, long)]
    cache: Option<String>,
    /// Overwrite existing files
    #[arg(short, long, name = "no-cache")]
    no_cache: bool,
    /// Limit download concurrency
    #[arg(short, long, default_value = "5")]
    limit: usize,
    #[command(flatten)]
    pub verbose: Verbosity<InfoLevel>,
    #[clap(skip)]
    pub multi: MultiProgress,
    #[clap(skip)]
    cleanup: Arc<CacheCleanup>,
}

impl Config {
    pub fn parse() -> Self {
        dotenv().ok();
        let config =  <Self as Parser>::parse();

        let info_level = config.verbose.log_level().unwrap() > log::Level::Info;
        let logger = env_logger::Builder::new()
          .format_timestamp(if info_level { None } else { Some(TimestampPrecision::Millis) }) 
          .format_target(info_level)
          .filter_level(config.verbose.log_level_filter()).build();

        let multi = MultiProgress::new();

        LogWrapper::new(multi.clone(), logger)
        .try_init()
        .unwrap();

        config
    }
    pub fn session(&self) -> String {
        if self.session.starts_with("FANBOXSESSID=") {
            self.session.clone()
        } else {
            format!("FANBOXSESSID={}", self.session)
        }
    }
    pub fn save_types(&self) -> SaveType {
        self.save
    }
    pub fn cache(&self) -> Option<PathBuf> {
        if self.no_cache {
            return None;
        };
        self.cache
            .clone()
            .or_else(|| Some(".".to_string()))
            .and_then(|s| Some(PathBuf::from(s)))
    }
    pub fn load_cache<T: DeserializeOwned>(&self, path: &str) -> Option<T> {
        let cache = self.cache()?;
        let path = cache.join(path);

        if path.exists() {
            info!("Loading cache {:?}", &path);
            let file = File::open(path).unwrap();
            let reader = BufReader::new(file);
            let data = serde_json::from_reader(reader).unwrap();
            Some(data)
        } else {
            None
        }
    }

    pub fn save_cache<T: Serialize>(&self, file: &str, data: &T) -> Option<()> {
        let cache = self.cache()?;
        let path = cache.join(file);
        let data = serde_json::to_vec(data).unwrap();
        self.cleanup.push(path, data);
        Some(())
    }

    pub fn output(&self) -> &PathBuf {
        &self.output
    }
    pub fn limit(&self) -> usize {
        self.limit
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Copy, Hash, ValueEnum, PartialEq, Eq)]
pub enum SaveType {
    All,
    Following,
    Supporting,
}

impl Default for SaveType {
    fn default() -> Self {
        SaveType::Supporting
    }
}

impl Display for SaveType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SaveType::All => write!(f, "all"),
            SaveType::Following => write!(f, "following"),
            SaveType::Supporting => write!(f, "supporting"),
        }
    }
}

#[derive(Debug, Default)]
struct CacheCleanup(Mutex<Vec<(PathBuf,Vec<u8>)>>);

impl CacheCleanup {
  pub fn push(&self, path: PathBuf, data: Vec<u8>) {
    self.0.lock().unwrap().push((path, data));
  }
}

impl Drop for CacheCleanup {
  fn drop(&mut self) {
    let data = self.0.lock().unwrap();
    for (path, data) in data.iter() {
      info!("Saving cache {:?}", &path);
      let mut file = File::create(path).unwrap();
      file.write_all(data).unwrap();
    }
  }
}