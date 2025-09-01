//! Хранилище состояния и настройка whitelists (с JSON-персистом).

use crate::config::Config;
use dashmap::{DashMap, DashSet};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use std::{fs, io};
use teloxide::types::{ChatId, User, UserId};
use crate::utils::normalize_username;

/// Ожидание прохождения капчи (в памяти, не пишется в файл).
#[derive(Clone)]
pub struct Pending {
    pub _user: u64,
    pub captcha_msg_id: i32,
    pub _deadline: Instant,
    pub user_message_ids: Vec<i32>,
}

/// То, что реально сохраняем на диск.
#[derive(Default, Serialize, Deserialize)]
struct PersistentState {
    // Боты
    bot_whitelist_ids: Vec<u64>,
    bot_whitelist_usernames: Vec<String>, // lower-case, без '@'
    // Люди
    user_whitelist_ids: Vec<u64>,
    user_whitelist_usernames: Vec<String>, // lower-case, без '@'
}

/// Простое файловое хранилище JSON.
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
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?; // гарантируем каталог
        }
        // atomic-ish запись
        let tmp = self.path.with_extension("tmp");
        let data = serde_json::to_vec_pretty(st).expect("serialize state");
        fs::write(&tmp, data)?;
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

/// Основное состояние приложения.
pub struct AppState {
    pub cfg: Config,

    /// Очередь ожидающих капчу: ключ (chat_id, user_id).
    pub pending: DashMap<(ChatId, u64), Pending>,

    // --- WHITELISTS (в памяти, зеркалим в JSON) ---
    // Боты
    pub bot_whitelist_ids: DashSet<u64>,
    pub bot_whitelist_names: DashSet<String>, // lower-case, без '@'
    // Люди
    pub user_whitelist_ids: DashSet<u64>,
    pub user_whitelist_names: DashSet<String>, // lower-case, без '@'

    store: Mutex<FileStore>,
}

impl AppState {
    pub fn new(cfg: Config) -> Self {
        let path: PathBuf = std::env::var("STATE_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("data/state.json"));
        let store = FileStore::new(path);

        let persisted = store.load().unwrap_or_default();

        // Восстанавливаем наборы из файла
        let bots_ids = DashSet::new();
        for id in persisted.bot_whitelist_ids {
            bots_ids.insert(id);
        }
        let bots_names = DashSet::new();
        for n in persisted.bot_whitelist_usernames {
            bots_names.insert(n.to_lowercase());
        }

        let users_ids = DashSet::new();
        for id in persisted.user_whitelist_ids {
            users_ids.insert(id);
        }
        let users_names = DashSet::new();
        for n in persisted.user_whitelist_usernames {
            users_names.insert(n.to_lowercase());
        }

        Self {
            cfg,
            pending: DashMap::new(),
            bot_whitelist_ids: bots_ids,
            bot_whitelist_names: bots_names,
            user_whitelist_ids: users_ids,
            user_whitelist_names: users_names,
            store: Mutex::new(store),
        }
    }

    #[inline]
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.cfg.captcha_timeout_secs)
    }

    #[inline]
    pub fn key(chat: ChatId, user: UserId) -> (ChatId, u64) {
        (chat, user.0)
    }

    // ---------- BOT WL ----------

    /// Разрешён ли бот (по id или по @username, если он есть).
    #[inline]
    pub fn is_bot_allowed_user(&self, user: &User) -> bool {
        if self.bot_whitelist_ids.contains(&user.id.0) {
            return true;
        }
        if let Some(name) = &user.username {
            return self.bot_whitelist_names.contains(&name.to_lowercase());
        }
        false
    }

    pub fn allow_bot_id(&self, id: u64) {
        self.bot_whitelist_ids.insert(id);
        self.persist();
    }
    pub fn deny_bot_id(&self, id: u64) {
        self.bot_whitelist_ids.remove(&id);
        self.persist();
    }
    pub fn allow_bot_username<S: AsRef<str>>(&self, name: S) {
        self.bot_whitelist_names.insert(normalize_username(name));
        self.persist();
    }
    pub fn deny_bot_username<S: AsRef<str>>(&self, name: S) {
        let n = normalize_username(name);
        self.bot_whitelist_names.remove(&n);
        self.persist();
    }

    // ---------- HUMAN WL ----------

    /// Разрешён ли пользователь (по id или по @username).
    #[inline]
    pub fn is_user_allowed(&self, user: &User) -> bool {
        if self.user_whitelist_ids.contains(&user.id.0) {
            return true;
        }
        if let Some(u) = &user.username {
            return self.user_whitelist_names.contains(&u.to_lowercase());
        }
        false
    }

    #[inline]
    pub fn allow_user_id(&self, id: u64) {
        self.user_whitelist_ids.insert(id);
        self.persist();
    }

    #[inline]
    pub fn deny_user_id(&self, id: u64) {
        self.user_whitelist_ids.remove(&id);
        self.persist();
    }

    #[inline]
    pub fn allow_username<S: AsRef<str>>(&self, name: S) {
        self.user_whitelist_names.insert(normalize_username(name));
        self.persist();
    }

    #[inline]
    pub fn deny_username<S: AsRef<str>>(&self, name: S) {
        let n = normalize_username(name);
        self.user_whitelist_names.remove(&n);
        self.persist();
    }

    /// Снимок и запись на диск.
    fn persist(&self) {
        let snapshot = PersistentState {
            bot_whitelist_ids: self.bot_whitelist_ids.iter().map(|x| *x).collect(),
            bot_whitelist_usernames: self.bot_whitelist_names.iter().map(|s| s.clone()).collect(),
            user_whitelist_ids: self.user_whitelist_ids.iter().map(|x| *x).collect(),
            user_whitelist_usernames: self.user_whitelist_names.iter().map(|s| s.clone()).collect(),
        };
        if let Ok(store) = self.store.lock() {
            let _ = store.save(&snapshot);
        }
    }
}
