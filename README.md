# Telegram Ranger

A Telegram bot to protect groups from spam.
It implements a **captcha** for new members, whitelist management, and automatic removal (kick/ban) of unverified users.

---

## Features

* Automatic captcha for new members
* Whitelists for users and bots (by numeric ID or `@username`)
* Captcha timeout → kick/ban (configurable)
* Optional deletion of messages sent by unverified users
* JSON-backed persistence for whitelists
* Admin-only command set

---

## Installation

1. Install Rust: [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install)

2. Clone the repository:

```bash
git clone https://github.com/filippovle/telegram-ranger.git
cd telegram-ranger
```

3. Create a `.env` file from the example:

```bash
cp .env.example .env
```

Edit `.env` and fill in your values.

4. Build:

```bash
cargo build --release
```

5. Run:

```bash
./target/release/telegram-ranger
```

> Tip (Windows): run in PowerShell from the project root so the bot can load `.env`.

---

## Configuration (.env)

| Variable                     | Required | Example             | Description                                                                                                  |
| ---------------------------- | -------- | ------------------- | ------------------------------------------------------------------------------------------------------------ |
| `TELOXIDE_TOKEN`             | yes      | `123456:ABC-DEF...` | Bot token from @BotFather                                                                                    |
| `ADMIN_USER_ID`              | yes      | `28324753`          | Your numeric Telegram user id; only this user can manage the bot                                             |
| `CAPTCHA_TIMEOUT_SEC`        | no       | `120`               | How long a new member has to pass the captcha                                                                |
| `KICK_BAN_MINUTES`           | no       | `10`                | Ban duration after timeout. `0` = short kick (ban+unban) to remove user immediately but allow instant rejoin |
| `DELETE_UNVERIFIED_MESSAGES` | no       | `true`              | Delete all messages authored by a user while they are pending captcha                                        |
| `STATE_FILE`                 | no       | `data/state.json`   | Where to store JSON state (whitelists)                                                                       |
| `RUST_LOG`                   | no       | `info`              | Logging level (e.g., `trace`, `debug`, `info`, `warn`, `error`)                                              |

See `.env.example` for a ready-to-edit template.

---

## Admin Commands

All admin commands work in private chat or group:

* `/allowbot <id|@username>` – allow a bot to join without captcha
* `/denybot <id|@username>` – remove bot from the allow-list
* `/allowuser <id|@username>` – allow a human to join without captcha
* `/denyuser <id|@username>` – remove human from the allow-list
* `/listallow` – show all allow-lists

Non-admins will receive a stub response or be ignored (configurable in code).

---

## How it works

* When a new member joins, the bot checks whitelists:

   * Bots not on the allow-list are banned immediately.
   * Whitelisted users/bots are let in without captcha.
   * Others receive a captcha message with a single button.
* If the user presses the button in time, they stay and get a welcome message.
* If the timer expires, the captcha message is removed and the user is kicked/banned according to `KICK_BAN_MINUTES`.
* If `DELETE_UNVERIFIED_MESSAGES=true`, the bot attempts to delete any messages sent by the user during the pending window.
* Whitelists persist across restarts in `STATE_FILE`.

---

## Building a small binary

Release profile already enables LTO and stripping via `Cargo.toml` (see `[profile.release]`).
You can build with:

```bash
cargo build --release
```

The resulting binary is in `target/release/telegram-ranger`.

---

## Systemd (optional)

Example unit:

```ini
[Unit]
Description=Telegram Ranger Bot
After=network.target

[Service]
WorkingDirectory=/opt/telegram-ranger
ExecStart=/opt/telegram-ranger/target/release/telegram-ranger
Environment=RUST_LOG=info
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

---

## Security notes

* Keep your `.env` out of version control. Use `.env.example` for templates.
* Only the `ADMIN_USER_ID` can modify allow-lists.
* Consider restricting who can add the bot to groups.

---

## License

This project is licensed under the [MIT License](LICENSE).

Copyright (c) 2025 Lev Filippov
