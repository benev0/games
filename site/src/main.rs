mod database;

use axum::{Router, extract::{FromRef, Path, Request, State}, http::StatusCode, middleware::{self, Next}, response::{Html, IntoResponse, Redirect, Response}, routing::{get, post}};
use axum_extra::extract::{CookieJar, Form, PrivateCookieJar, cookie::{Cookie, Key}};
use axum_htmx::HxBoosted;
use minijinja::{Environment, path_loader};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite};
use tokio::net::TcpListener;
use once_cell::sync::Lazy;

use minijinja::context;
use tower_http::{catch_panic::CatchPanicLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::database::{create_game, create_user, get_games, login_user, make_user_admin, user_is_admin};

static ENV: Lazy<Environment<'static>> = Lazy::new(|| {
    let mut env = Environment::new();
    env.set_loader(path_loader("templates"));
    env
});

#[derive(Clone)]
struct SiteState {
    key: Key,
    database: Pool<Sqlite>
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

#[derive(Deserialize, Debug)]
struct Name {
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
        // todo: require no auth?

    let protected_routes = Router::new()
        .route("/", get(games))
        .route("/profile", get(profile))
        .route("/settings", get(settings))
        .route("/games/{game}", get(specific_game))
        .route("/admin", get(admin))
        .route("/admin", post(make_admin))
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(state.clone(), require_auth));

    let admin_routes = Router::new()
        .route("/game", get(admin_game))
        .route("/game", post(admin_make_game))
        // .route("/end-code", method_router)
        // .route("/event", )
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(state, require_admin));

    let app = Router::new()
        .nest("/admin", admin_routes)
        .merge(protected_routes)
        .merge(public_routes)


        // todo: protect csrf
        .layer(CatchPanicLayer::new())
        .layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();

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
    let auth = jar.get("auth")
        // .and_then(|cookie| PrivateCookieJar::new(key).decrypt(cookie.clone()))
        .ok_or(anyhow::Error::msg("failed to retrieve valid cookie"))?;

    auth
        .value()
        .parse()
        .map_err(anyhow::Error::from)
}

async fn require_admin(
    State(state): State<SiteState>,
    jar: CookieJar,
    request: Request,
    next: Next
) -> Response {
    let auth = if let Some(auth) = jar.get("auth")
        .and_then(|cookie| PrivateCookieJar::new(state.key).decrypt(cookie.clone()))
    {
        auth
    } else {
        return Redirect::to("/login").into_response()
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
    next: Next
) -> Response {
    let _auth = if let Some(auth) = jar.get("auth")
        .and_then(|cookie| PrivateCookieJar::new(state.key).decrypt(cookie.clone()))
    {
        auth
    } else {
        return Redirect::to("/login").into_response()
    };

    // todo: check db

    next.run(request).await
}

async fn login_submit(
    State(state): State<SiteState>,
    HxBoosted(hx_boosted): HxBoosted,
    jar: PrivateCookieJar,
    Form(login_data): Form<Login>
) -> (PrivateCookieJar, Response) {
    // give cookie and redirect to "/"" on success (htmx and standard flavors)
    // stay on login on failure (htmx and standard flavors)

    match login_user(&state.database, login_data.uname, login_data.passwd).await {
        Ok(uid) => {
            let updated_jar = jar.add(Cookie::build(("auth", uid.to_string()))
                .http_only(true)
                .same_site(axum_extra::extract::cookie::SameSite::Strict)
                .build());

            (updated_jar, Redirect::to("/").into_response())
        },
        Err(_) => (jar, decide_htmx(hx_boosted, "login", context! {}).into_response()),
    }
}

async fn signup_submit(
    State(state): State<SiteState>,
    HxBoosted(hx_boosted): HxBoosted,
    jar: PrivateCookieJar,
    Form(signup_data): Form<Signup>
) -> (PrivateCookieJar, Response) {
    // give cookie and redirect to "/"" on success (htmx and standard flavors)
    // stay on login on failure (htmx and standard flavors)
    if signup_data.passwd != signup_data.confirm_passwd {
        return (jar, decide_htmx(hx_boosted, "signup", context! {}).into_response());
    }

    match create_user(&state.database, signup_data.uname, signup_data.passwd).await {
        Ok(uid) => {
            let updated_jar = jar.add(Cookie::build(("auth", uid.to_string()))
                .http_only(true)
                .same_site(axum_extra::extract::cookie::SameSite::Strict)
                .build());

            (updated_jar, Redirect::to("/").into_response())
        },
        Err(_) => (jar, decide_htmx(hx_boosted, "signup", context! {}).into_response()),
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

async fn profile(HxBoosted(hx_boosted): HxBoosted) -> Html<String> {
    decide_htmx(hx_boosted, "profile", context! {})
}

async fn settings(HxBoosted(hx_boosted): HxBoosted) -> Html<String> {
    decide_htmx(hx_boosted, "settings", context! {})
}

async fn specific_game(Path(game_name): Path<String>, HxBoosted(hx_boosted): HxBoosted) -> Html<String> {
    // todo: fetch content from db
    decide_htmx(hx_boosted, "games_", context! { game => game_name })
}

async fn admin_game(HxBoosted(hx_boosted): HxBoosted) -> Html<String> {
    decide_htmx(hx_boosted, "admin/game", context! {})
}

async fn admin_make_game(
    State(state): State<SiteState>,
    HxBoosted(_hx_boosted): HxBoosted,
    Form(game): Form<Name>
) -> StatusCode {
    match create_game(&state.database, game.name).await {
        Ok(_) => StatusCode::CREATED,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn admin(
    State(state): State<SiteState>,
    HxBoosted(hx_boosted): HxBoosted,
    jar: PrivateCookieJar
) -> Html<String> {
    let id = get_user_id(jar).unwrap();
    let user_is_admin = user_is_admin(&state.database, id)
        .await
        .unwrap_or(false);

    decide_htmx(hx_boosted, "admin", context! {is_admin => user_is_admin})
}

async fn make_admin(
    State(state): State<SiteState>,
    HxBoosted(hx_boosted): HxBoosted,
    jar: PrivateCookieJar
) -> Html<String> {
    let id = get_user_id(jar).unwrap();

    let success = make_user_admin(&state.database, id).await.is_ok();

    decide_htmx(hx_boosted, "admin", context! {is_admin => success})
}

fn decide_htmx<S>(htmx: bool, template: &str, ctx: S) -> Html<String>
where S: Serialize {
    let template = match htmx {
        true => ENV.get_template(&format!("partials/{}.html", template)),
        false => ENV.get_template(&format!("pages/{}.html", template)),
    };

    // todo: or 500 (switch to status result<html, status code>)
    let template = template.unwrap();
    Html(template.render(ctx).unwrap())
}
