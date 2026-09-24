use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct PublicUser {
    pub(crate) id: i64,
    pub(crate) username: String,
    pub(crate) locale: String,
}

mod server {
    use super::PublicUser;
    #[allow(unused_imports)]
    use dioxus::fullstack::{HeaderMap, SetCookie, SetHeader};
    use dioxus::prelude::*;

    #[cfg(feature = "server")]
    mod backend {
        use super::*;
        use dioxus::fullstack::headers::{Cookie, HeaderMapExt};
        use dioxus::fullstack::http::header::{COOKIE, ORIGIN};
        use dioxus::fullstack::{HeaderMap, SetCookie, SetHeader};
        use ring::{digest, pbkdf2, rand};
        use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
        use std::env;
        use std::num::NonZeroU32;
        use std::time::{SystemTime, UNIX_EPOCH};

        const COOKIE_NAME: &str = "zp_session";
        const SESSION_SECONDS: i64 = 7 * 24 * 60 * 60;
        const ITERATIONS: u32 = 600_000;
        const LOCAL_ORIGIN: &str = "http://127.0.0.1:8080";

        #[derive(Debug)]
        pub(super) enum AuthError {
            Invalid,
            Duplicate,
            Credentials,
            Unauthorized,
            Forbidden,
            Internal(String),
        }

        impl AuthError {
            pub(super) fn into_server_error(self) -> ServerFnError {
                let (code, message) = match self {
                    Self::Invalid => (400, "invalid_input".to_string()),
                    Self::Duplicate => (409, "username_taken".to_string()),
                    Self::Credentials => (401, "invalid_credentials".to_string()),
                    Self::Unauthorized => (401, "login_required".to_string()),
                    Self::Forbidden => (403, "invalid_origin".to_string()),
                    Self::Internal(detail) => {
                        eprintln!("Authentication operation failed: {detail}");
                        (500, "internal_error".to_string())
                    }
                };
                ServerFnError::ServerError {
                    message,
                    code,
                    details: None,
                }
            }
        }

        pub(super) fn db_error(error: impl std::fmt::Display) -> AuthError {
            AuthError::Internal(error.to_string())
        }

        pub(super) fn now() -> Result<i64, AuthError> {
            let duration = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(db_error)?;
            i64::try_from(duration.as_secs()).map_err(db_error)
        }

        pub(super) fn origin_config() -> Result<(String, bool), String> {
            let value = match env::var("ZETA_PRACTICE_PUBLIC_ORIGIN") {
                Ok(value) => value,
                Err(env::VarError::NotPresent) => LOCAL_ORIGIN.to_string(),
                Err(env::VarError::NotUnicode(_)) => {
                    return Err("ZETA_PRACTICE_PUBLIC_ORIGIN must be UTF-8".into());
                }
            };
            parse_origin(value)
        }

        fn parse_origin(value: String) -> Result<(String, bool), String> {
            let uri: dioxus::fullstack::http::Uri =
                value.parse().map_err(|_| "invalid public origin")?;
            let scheme = uri.scheme_str().ok_or("public origin needs a scheme")?;
            let authority = uri.authority().ok_or("public origin needs a host")?;
            if !matches!(scheme, "http" | "https")
                || authority.as_str().contains('@')
                || uri.path() != "/"
                || uri.query().is_some()
                || value.ends_with('/')
            {
                return Err(
                    "ZETA_PRACTICE_PUBLIC_ORIGIN must be an HTTP(S) origin without a path".into(),
                );
            }
            if scheme == "http" && !matches!(authority.host(), "127.0.0.1" | "localhost") {
                return Err("non-local public origins must use HTTPS".into());
            }
            Ok((value, scheme == "https"))
        }

        pub(crate) fn validate_origin_config() -> Result<(), String> {
            origin_config().map(|_| ())
        }

        pub(super) fn check_origin(headers: &HeaderMap, expected: &str) -> Result<(), AuthError> {
            let actual = headers.get(ORIGIN).and_then(|value| value.to_str().ok());
            if actual == Some(expected) {
                Ok(())
            } else {
                Err(AuthError::Forbidden)
            }
        }

        pub(super) fn token_from_headers(headers: &HeaderMap) -> Option<String> {
            if !headers.contains_key(COOKIE) {
                return None;
            }
            let token = headers.typed_get::<Cookie>()?.get(COOKIE_NAME)?.to_string();
            if token.len() == 64 && token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                Some(token)
            } else {
                None
            }
        }

        pub(super) fn cookie(
            token: Option<&str>,
            secure: bool,
        ) -> Result<SetHeader<SetCookie>, AuthError> {
            let mut value = format!(
                "{COOKIE_NAME}={}; Path=/; HttpOnly; SameSite=Lax",
                token.unwrap_or("")
            );
            if token.is_some() {
                value.push_str("; Max-Age=604800");
            } else {
                value.push_str("; Max-Age=0");
            }
            if secure {
                value.push_str("; Secure");
            }
            SetHeader::new(value).map_err(db_error)
        }

        fn random<const N: usize>() -> Result<[u8; N], AuthError> {
            let mut bytes = [0; N];
            rand::SecureRandom::fill(&rand::SystemRandom::new(), &mut bytes).map_err(db_error)?;
            Ok(bytes)
        }

        fn hex(bytes: &[u8]) -> String {
            const DIGITS: &[u8; 16] = b"0123456789abcdef";
            let mut encoded = String::with_capacity(bytes.len() * 2);
            for byte in bytes {
                encoded.push(DIGITS[(byte >> 4) as usize] as char);
                encoded.push(DIGITS[(byte & 15) as usize] as char);
            }
            encoded
        }

        fn unhex(value: &str) -> Option<Vec<u8>> {
            let mut bytes = Vec::with_capacity(value.len() / 2);
            let mut chunks = value.as_bytes().chunks_exact(2);
            for chunk in &mut chunks {
                let hi = (chunk[0] as char).to_digit(16)?;
                let lo = (chunk[1] as char).to_digit(16)?;
                bytes.push(((hi << 4) | lo) as u8);
            }
            if chunks.remainder().is_empty() {
                Some(bytes)
            } else {
                None
            }
        }

        fn valid_username(username: &str) -> bool {
            (3..=32).contains(&username.len())
                && username
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        }

        fn valid_password(password: &str) -> bool {
            (12..=128).contains(&password.len())
        }

        fn password_hash(password: &str) -> Result<String, AuthError> {
            let salt = random::<16>()?;
            let mut derived = [0; 32];
            pbkdf2::derive(
                pbkdf2::PBKDF2_HMAC_SHA256,
                NonZeroU32::new(ITERATIONS).expect("positive iteration count"),
                &salt,
                password.as_bytes(),
                &mut derived,
            );
            Ok(format!(
                "pbkdf2-sha256${ITERATIONS}${}${}",
                hex(&salt),
                hex(&derived)
            ))
        }

        fn verify_password(password: &str, encoded: &str) -> bool {
            let mut pieces = encoded.split('$');
            let (Some("pbkdf2-sha256"), Some(iterations), Some(salt), Some(hash), None) = (
                pieces.next(),
                pieces.next(),
                pieces.next(),
                pieces.next(),
                pieces.next(),
            ) else {
                return false;
            };
            let Some(iterations) = iterations.parse::<u32>().ok().and_then(NonZeroU32::new) else {
                return false;
            };
            let (Some(salt), Some(hash)) = (unhex(salt), unhex(hash)) else {
                return false;
            };
            salt.len() == 16
                && hash.len() == 32
                && pbkdf2::verify(
                    pbkdf2::PBKDF2_HMAC_SHA256,
                    iterations,
                    &salt,
                    password.as_bytes(),
                    &hash,
                )
                .is_ok()
        }

        fn token_hash(token: &str) -> [u8; 32] {
            let digest = digest::digest(&digest::SHA256, token.as_bytes());
            let mut hash = [0; 32];
            hash.copy_from_slice(digest.as_ref());
            hash
        }

        fn new_session(
            connection: &Connection,
            user_id: i64,
            current_time: i64,
        ) -> Result<String, AuthError> {
            let token = hex(&random::<32>()?);
            connection.execute(
                "INSERT INTO auth_sessions (token_hash, user_id, created_at, expires_at) VALUES (?1, ?2, ?3, ?4)",
                params![token_hash(&token).as_slice(), user_id, current_time, current_time + SESSION_SECONDS],
            ).map_err(db_error)?;
            Ok(token)
        }

        pub(super) fn register_account(
            connection: &mut Connection,
            username: &str,
            password: &str,
            locale: &str,
            current_time: i64,
        ) -> Result<(PublicUser, String), AuthError> {
            if !valid_username(username)
                || !valid_password(password)
                || !matches!(locale, "zh-CN" | "en-US")
            {
                return Err(AuthError::Invalid);
            }
            let hash = password_hash(password)?;
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(db_error)?;
            let inserted = transaction.execute(
                "INSERT INTO users (username, password_hash, created_at) VALUES (?1, ?2, ?3)",
                params![username, hash, current_time],
            );
            if let Err(error) = inserted {
                if matches!(error, rusqlite::Error::SqliteFailure(ref failure, _) if failure.code == rusqlite::ErrorCode::ConstraintViolation)
                {
                    return Err(AuthError::Duplicate);
                }
                return Err(db_error(error));
            }
            let user_id = transaction.last_insert_rowid();
            transaction
                .execute(
                    "INSERT INTO user_settings (user_id, locale) VALUES (?1, ?2)",
                    params![user_id, locale],
                )
                .map_err(db_error)?;
            let token = new_session(&transaction, user_id, current_time)?;
            transaction.commit().map_err(db_error)?;
            Ok((
                PublicUser {
                    id: user_id,
                    username: username.into(),
                    locale: locale.into(),
                },
                token,
            ))
        }

        pub(super) fn login_account(
            connection: &Connection,
            username: &str,
            password: &str,
            current_time: i64,
        ) -> Result<(PublicUser, String), AuthError> {
            if !valid_username(username) || !valid_password(password) {
                return Err(AuthError::Credentials);
            }
            let account = connection.query_row(
                "SELECT users.id, users.username, users.password_hash, user_settings.locale FROM users JOIN user_settings ON user_settings.user_id = users.id WHERE users.username = ?1",
                [username],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?)),
            ).optional().map_err(db_error)?;
            let Some((id, username, hash, locale)) = account else {
                let _ = password_hash(password)?;
                return Err(AuthError::Credentials);
            };
            if !verify_password(password, &hash) {
                return Err(AuthError::Credentials);
            }
            let token = new_session(connection, id, current_time)?;
            Ok((
                PublicUser {
                    id,
                    username,
                    locale,
                },
                token,
            ))
        }

        pub(super) fn current_user_for_token(
            connection: &Connection,
            token: Option<&str>,
            current_time: i64,
        ) -> Result<Option<PublicUser>, AuthError> {
            let Some(token) = token else {
                return Ok(None);
            };
            if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Ok(None);
            }
            connection.query_row(
                "SELECT users.id, users.username, user_settings.locale FROM auth_sessions JOIN users ON users.id = auth_sessions.user_id JOIN user_settings ON user_settings.user_id = users.id WHERE auth_sessions.token_hash = ?1 AND auth_sessions.revoked_at IS NULL AND auth_sessions.expires_at > ?2",
                params![token_hash(token).as_slice(), current_time],
                |row| Ok(PublicUser { id: row.get(0)?, username: row.get(1)?, locale: row.get(2)? }),
            ).optional().map_err(db_error)
        }

        pub(super) fn require_user(
            connection: &Connection,
            token: Option<&str>,
            current_time: i64,
        ) -> Result<PublicUser, AuthError> {
            current_user_for_token(connection, token, current_time)?.ok_or(AuthError::Unauthorized)
        }

        pub(super) fn logout_session(
            connection: &Connection,
            token: Option<&str>,
            current_time: i64,
        ) -> Result<(), AuthError> {
            let token = token.ok_or(AuthError::Unauthorized)?;
            let changed = connection.execute(
                "UPDATE auth_sessions SET revoked_at = ?1 WHERE token_hash = ?2 AND revoked_at IS NULL",
                params![current_time, token_hash(token).as_slice()],
            ).map_err(db_error)?;
            if changed == 1 {
                Ok(())
            } else {
                Err(AuthError::Unauthorized)
            }
        }

        pub(super) async fn run_db<T: Send + 'static>(
            operation: impl FnOnce(&mut Connection) -> Result<T, AuthError> + Send + 'static,
        ) -> Result<T, AuthError> {
            tokio::task::spawn_blocking(move || {
                let mut connection = crate::storage::open_from_env().map_err(db_error)?;
                operation(&mut connection)
            })
            .await
            .map_err(db_error)?
        }

        #[cfg(test)]
        mod tests {
            use super::*;

            fn database() -> Connection {
                let connection = Connection::open_in_memory().expect("memory database");
                connection
                    .pragma_update(None, "foreign_keys", "ON")
                    .expect("foreign keys");
                connection
                    .execute_batch(include_str!("../migrations/0002_accounts.sql"))
                    .expect("accounts schema");
                connection
            }

            #[test]
            fn accounts_sessions_and_revocation_are_isolated() {
                let mut connection = database();
                let (alice, alice_token) = register_account(
                    &mut connection,
                    "Alice",
                    "correct horse battery",
                    "zh-CN",
                    100,
                )
                .expect("alice");
                let (bob, bob_token) = register_account(
                    &mut connection,
                    "bob",
                    "different secret password",
                    "en-US",
                    100,
                )
                .expect("bob");
                assert!(matches!(
                    register_account(
                        &mut connection,
                        "alice",
                        "another long password",
                        "en-US",
                        100
                    ),
                    Err(AuthError::Duplicate)
                ));
                assert!(matches!(
                    login_account(&connection, "Alice", "wrong password", 101),
                    Err(AuthError::Credentials)
                ));
                let (alice_login, second_token) =
                    login_account(&connection, "ALICE", "correct horse battery", 101)
                        .expect("case-insensitive login");
                assert_eq!(alice_login.id, alice.id);
                assert_ne!(second_token, alice_token);
                assert_eq!(
                    require_user(&connection, Some(&bob_token), 101)
                        .expect("bob session")
                        .id,
                    bob.id
                );
                assert!(matches!(
                    require_user(&connection, None, 101),
                    Err(AuthError::Unauthorized)
                ));
                logout_session(&connection, Some(&alice_token), 102).expect("logout alice");
                assert!(
                    current_user_for_token(&connection, Some(&alice_token), 102)
                        .expect("revoked lookup")
                        .is_none()
                );
                assert_eq!(
                    require_user(&connection, Some(&second_token), 102)
                        .expect("other session")
                        .id,
                    alice.id
                );
                assert!(
                    current_user_for_token(&connection, Some(&bob_token), 100 + SESSION_SECONDS)
                        .expect("expired lookup")
                        .is_none()
                );
                let stored: Vec<u8> = connection
                    .query_row(
                        "SELECT token_hash FROM auth_sessions WHERE user_id = ?1 LIMIT 1",
                        [alice.id],
                        |row| row.get(0),
                    )
                    .expect("stored hash");
                assert_eq!(stored.len(), 32);
                assert_ne!(stored, alice_token.as_bytes());
            }

            #[test]
            fn origin_and_cookie_contract() {
                let mut headers = HeaderMap::new();
                headers.insert(ORIGIN, "http://127.0.0.1:8080".parse().expect("header"));
                assert!(check_origin(&headers, LOCAL_ORIGIN).is_ok());
                assert!(check_origin(&headers, "https://example.com").is_err());
                headers.remove(ORIGIN);
                assert!(check_origin(&headers, LOCAL_ORIGIN).is_err());
                let token = "a".repeat(64);
                headers.insert(
                    COOKIE,
                    format!("zp_session={token}")
                        .parse()
                        .expect("cookie request"),
                );
                assert_eq!(
                    token_from_headers(&headers).as_deref(),
                    Some(token.as_str())
                );
                headers.insert(COOKIE, "zp_session=short".parse().expect("short cookie"));
                assert!(token_from_headers(&headers).is_none());
                let secure = cookie(Some(&token), true).expect("secure cookie");
                let response = dioxus::fullstack::response::IntoResponse::into_response(secure);
                let header = response
                    .headers()
                    .get("set-cookie")
                    .expect("cookie header")
                    .to_str()
                    .expect("text");
                assert!(header.contains("HttpOnly"));
                assert!(header.contains("SameSite=Lax"));
                assert!(header.contains("Secure"));
                assert!(!header.contains("Domain="));
                assert!(
                    parse_origin("https://example.com".into())
                        .expect("HTTPS origin")
                        .1
                );
                assert!(!parse_origin(LOCAL_ORIGIN.into()).expect("local origin").1);
                assert!(parse_origin("http://example.com".into()).is_err());
                assert!(parse_origin("https://example.com/path".into()).is_err());
            }

            #[test]
            fn account_survives_connection_restart() {
                let path = std::env::temp_dir().join(format!(
                    "zeta-practice-account-{}-{}.sqlite",
                    std::process::id(),
                    now().expect("current time")
                ));
                let mut connection = Connection::open(&path).expect("open database");
                connection
                    .pragma_update(None, "foreign_keys", "ON")
                    .expect("foreign keys");
                connection
                    .execute_batch(include_str!("../migrations/0002_accounts.sql"))
                    .expect("schema");
                let (_, token) = register_account(
                    &mut connection,
                    "persisted",
                    "correct horse battery",
                    "en-US",
                    100,
                )
                .expect("register");
                drop(connection);
                let connection = Connection::open(&path).expect("reopen database");
                assert_eq!(
                    require_user(&connection, Some(&token), 101)
                        .expect("session after restart")
                        .username,
                    "persisted"
                );
                assert!(
                    login_account(&connection, "persisted", "correct horse battery", 101).is_ok()
                );
                drop(connection);
                std::fs::remove_file(path).expect("remove test database");
            }
        }
    }

    #[cfg(feature = "server")]
    pub(crate) use backend::validate_origin_config;

    /// Resolve the caller for a protected server operation; never accept a user ID from its payload.
    #[cfg(feature = "server")]
    pub(crate) async fn require_current_user(
        headers: &HeaderMap,
    ) -> Result<PublicUser, ServerFnError> {
        let token = token_from_headers(headers);
        run_db(move |connection| require_user(connection, token.as_deref(), now()?))
            .await
            .map_err(AuthError::into_server_error)
    }
    #[cfg(feature = "server")]
    use backend::*;

    #[post("/api/auth/register", headers: HeaderMap)]
    pub(crate) async fn register(
        username: String,
        password: String,
        locale: String,
    ) -> Result<(SetHeader<SetCookie>, dioxus::fullstack::Json<PublicUser>), ServerFnError> {
        let (origin, secure) = origin_config()
            .map_err(db_error)
            .map_err(AuthError::into_server_error)?;
        check_origin(&headers, &origin).map_err(AuthError::into_server_error)?;
        let result = run_db(move |connection| {
            register_account(connection, &username, &password, &locale, now()?)
        })
        .await
        .map_err(AuthError::into_server_error)?;
        Ok((
            cookie(Some(&result.1), secure).map_err(AuthError::into_server_error)?,
            dioxus::fullstack::Json(result.0),
        ))
    }

    #[post("/api/auth/login", headers: HeaderMap)]
    pub(crate) async fn login(
        username: String,
        password: String,
    ) -> Result<(SetHeader<SetCookie>, dioxus::fullstack::Json<PublicUser>), ServerFnError> {
        let (origin, secure) = origin_config()
            .map_err(db_error)
            .map_err(AuthError::into_server_error)?;
        check_origin(&headers, &origin).map_err(AuthError::into_server_error)?;
        let result =
            run_db(move |connection| login_account(connection, &username, &password, now()?))
                .await
                .map_err(AuthError::into_server_error)?;
        Ok((
            cookie(Some(&result.1), secure).map_err(AuthError::into_server_error)?,
            dioxus::fullstack::Json(result.0),
        ))
    }

    #[post("/api/auth/logout", headers: HeaderMap)]
    pub(crate) async fn logout() -> Result<SetHeader<SetCookie>, ServerFnError> {
        let (origin, secure) = origin_config()
            .map_err(db_error)
            .map_err(AuthError::into_server_error)?;
        check_origin(&headers, &origin).map_err(AuthError::into_server_error)?;
        require_current_user(&headers).await?;
        let token = token_from_headers(&headers);
        run_db(move |connection| logout_session(connection, token.as_deref(), now()?))
            .await
            .map_err(AuthError::into_server_error)?;
        cookie(None, secure).map_err(AuthError::into_server_error)
    }

    #[get("/api/auth/current-user", headers: HeaderMap)]
    pub(crate) async fn current_user() -> Result<Option<PublicUser>, ServerFnError> {
        let token = token_from_headers(&headers);
        run_db(move |connection| current_user_for_token(connection, token.as_deref(), now()?))
            .await
            .map_err(AuthError::into_server_error)
    }
}

#[cfg(feature = "web")]
pub(crate) use server::current_user;
#[cfg(feature = "server")]
pub(crate) use server::validate_origin_config;
pub(crate) use server::{login, logout, register};
