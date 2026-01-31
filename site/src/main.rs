use axum::{Router, extract::Path, response::Html, routing::get};
use axum_htmx::HxBoosted;
use minijinja::{Environment, path_loader};
use serde::Serialize;
use tokio::net::TcpListener;
use once_cell::sync::Lazy;

use minijinja::context;
use tower_http::{catch_panic::CatchPanicLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

static ENV: Lazy<Environment<'static>> = Lazy::new(|| {
    let mut env = Environment::new();
    env.set_loader(path_loader("templates"));
    env
});


#[tokio::main]
async fn main() {
    init_tracing();

    let public_routes = Router::new()
        .route("/login", get(Html("<h1>login</h1>")));
        // todo: require no auth


    let protected_routes = Router::new()
        .route("/", get(games))
        .route("/profile", get(profile))
        .route("/settings", get(settings))
        .route("/games/{game}", get(specific_game));
        // todo: require auth

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
