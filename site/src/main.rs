mod database;

use axum::{Router, extract::{FromRef, Path, Request, State}, middleware::{self, Next}, response::{Html, IntoResponse, Redirect, Response}, routing::{get, post}};
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

use crate::database::{create_user, login_user};

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
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(state, require_auth));

    let app = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
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

async fn games(HxBoosted(hx_boosted): HxBoosted) -> Html<String> {
    decide_htmx(hx_boosted, "games", context! { games => vec!["gravitrips", "chess"]})
}

async fn profile(HxBoosted(hx_boosted): HxBoosted) -> Html<String> {
    decide_htmx(hx_boosted, "profile", context! {})
}

async fn settings(HxBoosted(hx_boosted): HxBoosted) -> Html<String> {
    decide_htmx(hx_boosted, "settings", context! {})
}

async fn specific_game(Path(game): Path<String>, HxBoosted(hx_boosted): HxBoosted) -> Html<String> {
    decide_htmx(hx_boosted, &format!("games/{}", game), context! {})
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
