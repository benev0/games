use std::str::FromStr;

use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core},
};
use chrono::Utc;
use serde::Serialize;
use sqlx::{
    Pool, Sqlite, query, query_scalar, sqlite::{self, SqlitePool}
};

#[derive(Serialize, Debug)]
pub(crate) struct Game {
    id: i64,
    name: String,
    description: String,
}

impl Game {
    fn new(id: i64, name: String, description: String) -> Self {
        Self { id, name, description}
    }
}

pub(crate) async fn initialize() -> anyhow::Result<Pool<Sqlite>> {
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

pub(crate) async fn create_user(
    pool: &Pool<Sqlite>,
    username: String,
    passwd: String,
) -> anyhow::Result<i64> {
    let salt = SaltString::generate(&mut rand_core::OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(passwd.as_bytes(), &salt)
        .map_err(|_| anyhow::Error::msg("argon2 hashing failed"))?;

    let mut transaction = pool.begin().await?;

    let time = Utc::now();

    let id = query!(
        "insert into user (username, created) values ( ?1, ?2 )",
        username,
        time
    )
    .execute(&mut *transaction)
    .await?
    .last_insert_rowid();

    let salt_string = salt.as_str();
    let hash_string = password_hash.to_string();

    query!(
        "insert into passwd ( user_id, algo, salt, pass_hash) values ( ?1, ?2, ?3, ?4 )",
        id,
        0,
        salt_string,
        hash_string
    )
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    Ok(id)
}

pub(crate) async fn login_user(
    pool: &Pool<Sqlite>,
    username: String,
    passwd: String,
) -> anyhow::Result<i64> {
    let mut conn = pool.acquire().await?;

    let rec = query!("select passwd.user_id, passwd.algo, passwd.options, passwd.salt, passwd.pass_hash from user inner join passwd on user.id = passwd.user_id where user.username = ?1", username)
        .fetch_one(&mut *conn)
        .await?;

    let hash_str = String::from_utf8_lossy(&rec.pass_hash);

    let parsed_hash = PasswordHash::new(&hash_str)
        .map_err(|_| anyhow::Error::msg("passwd hash failed to parse"))?;
    Argon2::default()
        .verify_password(passwd.as_bytes(), &parsed_hash)
        .map_err(|_| anyhow::Error::msg("passwd failed to verify"))?;

    Ok(rec.user_id)
}

pub(crate) async fn get_games(pool: &Pool<Sqlite>) -> anyhow::Result<Vec<Game>> {
    let mut conn = pool.acquire().await?;

    let games: Vec<Game> = query!("select * from game")
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|rec| Game::new(rec.id, rec.game_name, rec.game_description))
        .collect();

    Ok(games)
}

pub(crate) async fn user_exists(pool: &Pool<Sqlite>, id: i64) -> anyhow::Result<bool> {
    let mut conn = pool.acquire().await?;

    let user_exists: _ = query_scalar!("select exists(select 1 from user where id = ?1) as \"exists!: bool\"", id)
        .fetch_one(&mut *conn)
        .await
        .unwrap_or(false);

    Ok(user_exists)
}

pub(crate) async fn get_username(pool: &Pool<Sqlite>, id: i64) -> anyhow::Result<String> {
    let mut conn = pool.acquire().await?;

    let username = query!("select username from user where id = ?1", id)
        .fetch_one(&mut *conn)
        .await?;

    Ok(username.username)
}

pub(crate) async fn make_user_admin(pool: &Pool<Sqlite>, id: i64) -> anyhow::Result<()> {
    let mut conn = pool.acquire().await?;

    query!("insert into administrator (user_id) values ( ?1 )", id)
        .execute(&mut *conn)
        .await?;

    Ok(())
}

pub(crate) async fn user_is_admin(pool: &Pool<Sqlite>, id: i64) -> anyhow::Result<bool> {
    let mut conn = pool.acquire().await?;

    let is_admin = query!("select * from administrator where user_id = ?1", id)
        .fetch_one(&mut *conn)
        .await
        .map(|req| req.user_id == id)
        .unwrap_or(false);

    Ok(is_admin)
}

pub(crate) async fn get_game_with_name(pool: &Pool<Sqlite>, name: &str) -> anyhow::Result<(i64, String)> {
    let mut conn = pool.acquire().await?;

    let res = query!("select * from game where game_name = ?1", name)
        .fetch_one(&mut *conn)
        .await?;

    Ok((res.id, res.game_description))
}

pub(crate) async fn get_event_with_name(pool: &Pool<Sqlite>, name: &str) -> anyhow::Result<(i64, String, i64, i64, String)> {
    let mut conn = pool.acquire().await?;

    let res = query!("select * from game_event where event_name = ?1", name)
        .fetch_optional(&mut *conn)
        .await?
        .unwrap();

    Ok((res.id, res.event_name, res.created, res.game_id, res.event_description))
}

pub(crate) async fn get_all_event_names(pool: &Pool<Sqlite>) -> anyhow::Result<Vec<String>> {
    let mut conn = pool.acquire().await?;

    let res = query!("select event_name from game_event")
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|e| e.event_name)
        .collect();

    Ok(res)
}

pub(crate) async fn get_game_event_names(pool: &Pool<Sqlite>, game: &str) -> anyhow::Result<Vec<String>> {
    let mut conn = pool.acquire().await?;

    let res = query!("select event_name from game_event inner join game on game_event.game_id = game.id where game.game_name = ?1", game)
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|e| e.event_name)
        .collect();

    Ok(res)
}

pub(crate) async fn create_end_code(pool: &Pool<Sqlite>, name: &str) -> anyhow::Result<i64> {
    let mut conn = pool.acquire().await?;

    let id = query!("insert into game_code (code) values ( ?1 )", name)
        .execute(&mut *conn)
        .await?
        .last_insert_rowid();

    Ok(id)
}

pub(crate) async fn create_event(pool: &Pool<Sqlite>, name: &str, for_game: i64, description: &str) -> anyhow::Result<i64> {
    let mut conn = pool.acquire().await?;

    let id = query!("insert into game_event (game_id, event_name, created, event_description) values ( ?1, ?2, ?3, ?4 )", for_game, name, 0, description)
        .execute(&mut *conn)
        .await?
        .last_insert_rowid();

    Ok(id)
}

pub(crate) async fn create_game(pool: &Pool<Sqlite>, name: &str, description: &str) -> anyhow::Result<i64> {
    let mut conn = pool.acquire().await?;

    let id = query!("insert into game (game_name, game_description) values ( ?1, ?2 )", name, description)
        .execute(&mut *conn)
        .await?
        .last_insert_rowid();

    Ok(id)
}
