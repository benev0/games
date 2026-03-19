mod database;

use axum::{
    Router,
    extract::{FromRef, Path, Request, State, Multipart},
    http::StatusCode,
    middleware::{self, Next},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use axum_extra::extract::{
    CookieJar, Form, PrivateCookieJar, cookie::{Cookie, Key}
};
use axum_htmx::HxBoosted;
use base64::{engine::general_purpose::URL_SAFE, Engine as _};
use minijinja::{Environment, path_loader};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_512};
use sqlx::{Pool, Sqlite};
use tokio::net::TcpListener;

use minijinja::context;
use tower_http::{catch_panic::CatchPanicLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::database::{
    create_end_code, create_event, create_game, create_user, create_user_submitted_bot_for_event, get_all_event_names, get_bots_from_event_and_user, get_bots_from_user, get_event_with_name, get_game_event_names, get_game_with_name, get_games, get_username, login_user, make_user_admin, user_exists, user_is_admin
};

static ENV: Lazy<Environment<'static>> = Lazy::new(|| {
    let mut env = Environment::new();
    env.set_loader(path_loader("templates"));
    env
});

#[derive(Clone)]
struct SiteState {
    key: Key,
    database: Pool<Sqlite>,
}

impl FromRef<SiteState> for Key {
    fn from_ref(state: &SiteState) -> Self {
        state.key.clone()
    }
}

#[derive(Deserialize, Debug)]
struct Login {
    uname: String,
    passwd: String,
}

#[derive(Deserialize, Debug)]
struct Signup {
    uname: String,
    passwd: String,
    confirm_passwd: String,
}

// forum data
#[derive(Deserialize, Debug)]
struct GameInfo {
    name: String,
    description: String,
}

// forum data
#[derive(Deserialize, Debug)]
struct EventInfo {
    name: String,
    game: i64,
    // add creation time
    description: String,
}

// forum data
#[derive(Deserialize, Debug)]
struct EndCodeInfo {
    name: String,
}

#[tokio::main]
async fn main() {
    init_tracing();

    let pool = database::initialize()
        .await
        .expect("database failed to open");

    let state = SiteState {
        key: Key::generate(),
        database: pool,
    };

    let public_routes = Router::new()
        .route("/login", get(login))
        .route("/login", post(login_submit))
        .route("/signup", get(signup))
        .route("/signup", post(signup_submit))
        .with_state(state.clone());

    let protected_routes = Router::new()
        .route("/", get(games))
        .route("/profile", get(profile))
        .route("/settings", get(settings))
        .route("/games/{game}", get(specific_game))
        .route("/events", get(events))
        .route("/event/{event_name}", get(event))
        .route("/event/{event_name}", post(submit_new_bot_for_event))
        .route("/adminme", get(admin))
        .route("/adminme", post(make_admin))
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(state.clone(), require_auth));

    let admin_routes = Router::new()
        .route("/game", get(admin_game))
        .route("/game", post(admin_make_game))
        .route("/event", get(admin_event))
        .route("/event", post(admin_make_event))
        .route("/end-code", get(admin_end_code))
        .route("/end-code", post(admin_make_end_code))
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(state, require_admin));

    let app = Router::new()
        .nest("/admin", admin_routes)
        .merge(protected_routes)
        .merge(public_routes)
        // todo: protect csrf better
        // use in order of preference low to high: origin headers, double cookie submit, or http3
        .layer(CatchPanicLayer::new())
        .layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();

    println!("listening on {}", listener.local_addr().unwrap());
    let _ = axum::serve(listener, app).await;
}

fn init_tracing() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("info,tower_http=trace"))
        .with(tracing_subscriber::fmt::layer())
        .init();
}

fn get_user_id(jar: PrivateCookieJar) -> anyhow::Result<i64> {
    let auth = jar
        .get("auth")
        .ok_or(anyhow::Error::msg("failed to retrieve valid cookie"))?;

    auth.value().parse().map_err(anyhow::Error::from)
}

async fn require_admin(
    State(state): State<SiteState>,
    jar: CookieJar,
    request: Request,
    next: Next,
) -> Response {
    let auth = if let Some(auth) = jar
        .get("auth")
        .and_then(|cookie| PrivateCookieJar::new(state.key).decrypt(cookie.clone()))
    {
        auth
    } else {
        return Redirect::to("/login").into_response();
    };

    if let Ok(auth) = auth.value().parse()
        && let Ok(true) = user_is_admin(&state.database, auth).await
    {
        next.run(request).await
    } else {
        StatusCode::BAD_REQUEST.into_response()
    }
}

async fn require_auth(
    State(state): State<SiteState>,
    jar: CookieJar,
    request: Request,
    next: Next,
) -> Response {
    let auth = if let Some(auth) = jar
        .get("auth")
        .and_then(|cookie| PrivateCookieJar::new(state.key).decrypt(cookie.clone()))
    {
        auth
    } else {
        return Redirect::to("/login").into_response();
    };

    if let Ok(auth) = auth.value().parse()
        && let Ok(true) = user_exists(&state.database, auth).await
    {
        next.run(request).await
    } else {
        Redirect::to("/login").into_response()
    }
}

async fn login_submit(
    State(state): State<SiteState>,
    HxBoosted(hx_boosted): HxBoosted,
    jar: PrivateCookieJar,
    Form(login_data): Form<Login>,
) -> (PrivateCookieJar, Response) {
    // give cookie and redirect to "/"" on success (htmx and standard flavors)
    // stay on login on failure (htmx and standard flavors)

    match login_user(&state.database, login_data.uname, login_data.passwd).await {
        Ok(uid) => {
            let updated_jar = jar.add(
                Cookie::build(("auth", uid.to_string()))
                    .http_only(true)
                    .same_site(axum_extra::extract::cookie::SameSite::Strict)
                    .build(),
            );

            (updated_jar, Redirect::to("/").into_response())
        }
        Err(_) => (
            jar,
            decide_htmx(hx_boosted, "login", context! {}).into_response(),
        ),
    }
}

async fn signup_submit(
    State(state): State<SiteState>,
    HxBoosted(hx_boosted): HxBoosted,
    jar: PrivateCookieJar,
    Form(signup_data): Form<Signup>,
) -> (PrivateCookieJar, Response) {
    // give cookie and redirect to "/" on success (htmx and standard flavors)
    // stay on login on failure (htmx and standard flavors)
    if signup_data.passwd != signup_data.confirm_passwd {
        return (
            jar,
            decide_htmx(hx_boosted, "signup", context! {}).into_response(),
        );
    }

    match create_user(&state.database, signup_data.uname, signup_data.passwd).await {
        Ok(uid) => {
            let updated_jar = jar.add(
                Cookie::build(("auth", uid.to_string()))
                    .http_only(true)
                    .same_site(axum_extra::extract::cookie::SameSite::Strict)
                    .build(),
            );

            (updated_jar, Redirect::to("/").into_response())
        }
        Err(_) => (
            jar,
            decide_htmx(hx_boosted, "signup", context! {}).into_response(),
        ),
    }
}

async fn login(HxBoosted(hx_boosted): HxBoosted) -> Html<String> {
    decide_htmx(hx_boosted, "login", context! {})
}

async fn signup(HxBoosted(hx_boosted): HxBoosted) -> Html<String> {
    decide_htmx(hx_boosted, "signup", context! {})
}

async fn games(State(state): State<SiteState>, HxBoosted(hx_boosted): HxBoosted) -> Html<String> {
    let game_list = get_games(&state.database).await.unwrap();
    decide_htmx(hx_boosted, "games", context! { games => game_list })
}

async fn profile(
    State(state): State<SiteState>,
    HxBoosted(hx_boosted): HxBoosted,
    jar: PrivateCookieJar,
) -> Html<String> {
    let id = get_user_id(jar).unwrap();
    let username = get_username(&state.database, id).await.unwrap_or("Unknown Username".to_owned());
    let bot_hashes: Vec<String> = get_bots_from_user(&state.database, id)
        .await
        .unwrap_or(Vec::new())
        .iter()
        .map(|bot| URL_SAFE.encode(&bot.bot_hash))
        .collect();

    decide_htmx(hx_boosted, "profile", context! { username => username, bot_submissions => bot_hashes })
}

async fn settings(HxBoosted(hx_boosted): HxBoosted) -> Html<String> {
    decide_htmx(hx_boosted, "settings", context! {})
}

async fn specific_game(
    State(state): State<SiteState>,
    Path(game_name): Path<String>,
    HxBoosted(hx_boosted): HxBoosted,
) -> Html<String> {
    // todo: fetch content from db
    let (_id, description) = get_game_with_name(&state.database, &game_name).await.unwrap_or((0, "game not found".to_string()));
    let event_names = get_game_event_names(&state.database, &game_name).await.unwrap();
    decide_htmx(hx_boosted, "games_", context! { game => game_name, description => description, events => event_names})
}

async fn events(
    State(state): State<SiteState>,
    HxBoosted(hx_boosted): HxBoosted,
) -> Html<String> {
    let event_names = get_all_event_names(&state.database).await.unwrap();
    decide_htmx(hx_boosted, "events", context! { events => event_names })
}

async fn event(
    State(state): State<SiteState>,
    Path(event_name): Path<String>,
    HxBoosted(hx_boosted): HxBoosted,
    jar: PrivateCookieJar,
) -> Html<String> {
    let user_id = get_user_id(jar).unwrap(); // should be a 400 range error on fail
    let event_data = get_event_with_name(&state.database, &event_name).await.unwrap();
    let submissions = get_bots_from_event_and_user(&state.database, event_data.id, user_id).await.unwrap();
    let available_bots = get_bots_from_user(&state.database, user_id).await.unwrap();
    decide_htmx(hx_boosted, "event_", context! { event => event_data, submissions => submissions, available_bots => available_bots })
}

async fn submit_new_bot_for_event(
    State(state): State<SiteState>,
    Path(event_name): Path<String>,
    HxBoosted(_hx_boosted): HxBoosted,
    jar: PrivateCookieJar,
    mut multipart: Multipart
) -> Html<String> {
    let user_id = get_user_id(jar).unwrap();

    while let Some(field) = multipart.next_field().await.unwrap() {
        if let Some("bot") = field.name() {
            let data = field.bytes().await.unwrap();
            let mut hasher = Sha3_512::new();
            hasher.update(&data);
            let hash = hasher.finalize();
            let filename = URL_SAFE.encode(hash);

            // fixme: most jank early return
            let _ = tokio::fs::write(format!("./bots/{}.wasm", filename), &data).await.unwrap();

            // todo: should also bind to event as well
            let _ = create_user_submitted_bot_for_event(&state.database, &hash, 0, user_id, &event_name).await.unwrap();
            return Html("success".to_string());
        }
    }
    Html("failure".to_string())
}

async fn admin_game(HxBoosted(hx_boosted): HxBoosted) -> Html<String> {
    decide_htmx(hx_boosted, "admin/game", context! {})
}

async fn admin_make_game(
    State(state): State<SiteState>,
    Form(game): Form<GameInfo>,
) -> StatusCode {
    match create_game(&state.database, &game.name, &game.description).await {
        Ok(_) => StatusCode::CREATED,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn admin_event(
    State(state): State<SiteState>,
    HxBoosted(hx_boosted): HxBoosted
) -> Html<String> {
    let games = get_games(&state.database).await.unwrap();
    decide_htmx(hx_boosted, "admin/event", context! { games => games })
}

async fn admin_make_event(
    State(state): State<SiteState>,
    Form(event): Form<EventInfo>,
) -> StatusCode {
    match create_event(&state.database, &event.name, event.game, &event.description).await {
        Ok(_) => StatusCode::CREATED,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn admin_end_code(
    HxBoosted(hx_boosted): HxBoosted
) -> Html<String> {
    decide_htmx(hx_boosted, "admin/end_code", context! {})
}

async fn admin_make_end_code(
    State(state): State<SiteState>,
    Form(end_code): Form<EndCodeInfo>,
) -> StatusCode {
    match create_end_code(&state.database, &end_code.name).await {
        Ok(_) => StatusCode::CREATED,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn admin(
    State(state): State<SiteState>,
    HxBoosted(hx_boosted): HxBoosted,
    jar: PrivateCookieJar,
) -> Html<String> {
    let id = get_user_id(jar).unwrap();
    let user_is_admin = user_is_admin(&state.database, id).await.unwrap_or(false);

    decide_htmx(hx_boosted, "admin", context! {is_admin => user_is_admin})
}

async fn make_admin(
    State(state): State<SiteState>,
    HxBoosted(hx_boosted): HxBoosted,
    jar: PrivateCookieJar,
) -> Html<String> {
    let id = get_user_id(jar).unwrap();

    let success = make_user_admin(&state.database, id).await.is_ok();

    decide_htmx(hx_boosted, "admin", context! {is_admin => success})
}

fn decide_htmx<S>(htmx: bool, template: &str, ctx: S) -> Html<String>
where
    S: Serialize,
{
    let template = match htmx {
        true => ENV.get_template(&format!("partials/{}.html", template)),
        false => ENV.get_template(&format!("pages/{}.html", template)),
    };

    // todo: or 500 (switch to status result<html, status code>)
    let template = template.unwrap();
    Html(template.render(ctx).unwrap())
}
