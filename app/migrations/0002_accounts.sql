CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    username TEXT NOT NULL COLLATE NOCASE UNIQUE,
    password_hash TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE user_settings (
    user_id INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    locale TEXT NOT NULL CHECK (locale IN ('zh-CN', 'en-US'))
);

CREATE TABLE auth_sessions (
    token_hash BLOB PRIMARY KEY CHECK (length(token_hash) = 32),
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    revoked_at INTEGER
);

CREATE INDEX auth_sessions_user_id ON auth_sessions(user_id);
