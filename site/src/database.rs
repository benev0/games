use std::str::FromStr;


use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::{SaltString, rand_core}};
use chrono::Utc;
use sqlx::{Pool, Sqlite, query, sqlite::{self, SqlitePool}};

pub (crate) async fn initialize() -> anyhow::Result<Pool<Sqlite>> {
    let options = sqlite::SqliteConnectOptions::from_str(&std::env::var("DATABASE_URL")?)?
        .foreign_keys(true)
        .create_if_missing(false)
        .busy_timeout(std::time::Duration::from_secs(5));

    let pool = SqlitePool::connect_with(options)
        .await
        .map_err(anyhow::Error::from)?;

    query("PRAGMA journal_mode = WAL;")
        .execute(&pool)
        .await
        .map_err(anyhow::Error::from)?;

    Ok(pool)
}

pub (crate) async fn create_user(pool: &Pool<Sqlite>, username: String, passwd: String) -> anyhow::Result<i64> {
    let salt = SaltString::generate(&mut rand_core::OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2.hash_password(passwd.as_bytes(), &salt)
        .map_err(|_| anyhow::Error::msg("argon2 hashing failed"))?;


    let mut transaction = pool.begin().await?;

    let time = Utc::now();

    let id = query!("insert into user (username, created) values ( ?1, ?2 )", username, time)
        .execute(&mut *transaction)
        .await?
        .last_insert_rowid();

    let salt_string = salt.as_str();
    let hash_string = password_hash.to_string();

    query!("insert into passwd ( user_id, algo, salt, pass_hash) values ( ?1, ?2, ?3, ?4 )", id, 0, salt_string, hash_string)
        .execute(&mut *transaction)
        .await?;

    transaction.commit().await?;

    Ok(id)
}


pub (crate) async fn login_user(pool: &Pool<Sqlite>, username: String, passwd: String) -> anyhow::Result<i64> {
    let mut conn = pool.acquire().await?;

    let rec = query!("select passwd.user_id, passwd.algo, passwd.options, passwd.salt, passwd.pass_hash from user inner join passwd on user.id = passwd.user_id where user.username = ?1", username)
        .fetch_one(&mut *conn)
        .await?;

    let hash_str = String::from_utf8_lossy(&rec.pass_hash);

    let parsed_hash = PasswordHash::new(&hash_str).map_err(|_| anyhow::Error::msg("passwd hash failed to parse"))?;
    Argon2::default().verify_password(passwd.as_bytes(), &parsed_hash).map_err(|_| anyhow::Error::msg("passwd failed to verify"))?;

    Ok(rec.user_id)
}

pub (crate) async fn get_games(pool: &Pool<Sqlite>) -> anyhow::Result<Vec<String>> {
    let mut conn = pool.acquire().await?;

    let games: Vec<String> = query!("select game_name from game")
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|rec| rec.game_name)
        .collect();

    Ok(games)
}

pub (crate) async fn user_is_admin(pool: &Pool<Sqlite>, id: i64) -> anyhow::Result<bool> {
    let mut conn = pool.acquire().await?;

    let is_admin = query!("select * from administrator where user_id = ?1", id)
        .fetch_one(&mut *conn)
        .await
        .map(|req| req.user_id == id)
        .unwrap_or(false);

    Ok(is_admin)
}

pub (crate) async fn create_game(pool: &Pool<Sqlite>, name: String) -> anyhow::Result<i64> {
    let mut conn = pool.acquire().await?;

    let id = query!("insert into game (game_name) values ( ?1 )", name)
        .execute(&mut *conn)
        .await?
        .last_insert_rowid();

    Ok(id)
}

pub (crate) async fn create_end_code(pool: &Pool<Sqlite>, name: String) -> anyhow::Result<i64> {
    let mut conn = pool.acquire().await?;

    let id = query!("insert into game_code (code) values ( ?1 )", name)
        .execute(&mut *conn)
        .await?
        .last_insert_rowid();

    Ok(id)
}
