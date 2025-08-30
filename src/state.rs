// src/state.rs
use crate::config::Config;
use dashmap::{DashMap, DashSet};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Mutex};
use std::time::{Duration, Instant};
use std::{fs, io};
use teloxide::types::{ChatId, UserId};

#[derive(Clone)]
pub struct Pending {
    pub _user: u64,
    pub captcha_msg_id: i32,
    pub _deadline: Instant,
}

#[derive(Default, Serialize, Deserialize)]
struct PersistentState {
    bot_whitelist: Vec<u64>,
}

struct FileStore {
    path: PathBuf,
}

impl FileStore {
    fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    fn load(&self) -> io::Result<PersistentState> {
        match fs::read_to_string(&self.path) {
            Ok(s) => Ok(serde_json::from_str(&s).unwrap_or_default()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(PersistentState::default()),
            Err(e) => Err(e),
        }
    }

    fn save(&self, st: &PersistentState) -> io::Result<()> {
        let tmp = self.path.with_extension("tmp");
        let data = serde_json::to_vec_pretty(st).expect("serialize state");
        fs::write(&tmp, data)?;
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

pub struct AppState {
    pub cfg: Config,
    pub pending: DashMap<(ChatId, u64), Pending>,
    pub bot_whitelist: DashSet<u64>,
    store: Mutex<FileStore>,
}

impl AppState {
    pub fn new(cfg: Config) -> Self {
        let path: PathBuf = std::env::var("STATE_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("state.json"));

        let store = FileStore::new(path);

        let persisted = store.load().unwrap_or_default();
        let whitelist = DashSet::new();
        for id in persisted.bot_whitelist {
            whitelist.insert(id);
        }

        Self {
            cfg,
            pending: DashMap::new(),
            bot_whitelist: whitelist,
            store: Mutex::new(store),
        }
    }

    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.cfg.captcha_timeout_secs)
    }

    #[inline]
    pub fn key(chat: ChatId, user: UserId) -> (ChatId, u64) {
        (chat, user.0)
    }

    #[inline]
    pub fn is_bot_allowed(&self, id: UserId) -> bool {
        self.bot_whitelist.contains(&id.0)
    }

    #[inline]
    pub fn allow_bot(&self, id: u64) {
        self.bot_whitelist.insert(id);
        self.persist();
    }

    #[inline]
    pub fn deny_bot(&self, id: u64) {
        self.bot_whitelist.remove(&id);
        self.persist();
    }

    fn persist(&self) {
        let snapshot = PersistentState {
            bot_whitelist: self.bot_whitelist.iter().map(|x| *x).collect(),
        };
        if let Ok(store) = self.store.lock() {
            let _ = store.save(&snapshot);
        }
    }
}
