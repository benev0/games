use axum::{Router, response::Html, routing::get};
use axum_htmx::HxBoosted;
use minijinja::{Environment, path_loader};
use tokio::net::TcpListener;
use once_cell::sync::Lazy;

use minijinja::context;

static ENV: Lazy<Environment<'static>> = Lazy::new(|| {
    let mut env = Environment::new();
    env.set_loader(path_loader("templates"));
    env
});


#[tokio::main]
async fn main() {

    let app = Router::new()
        .route("/", get(games))
        .route("/profile", get(profile))
        .route("/settings", get(settings));

    let listener = TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();

    println!("listening on {}", listener.local_addr().unwrap());
    let _ = axum::serve(listener, app).await;
}

async fn games(HxBoosted(hx_boosted): HxBoosted) -> Html<String> {
    decide_htmx(hx_boosted, "games")
}

async fn profile(HxBoosted(hx_boosted): HxBoosted) -> Html<String> {
    decide_htmx(hx_boosted, "profile")
}

async fn settings(HxBoosted(hx_boosted): HxBoosted) -> Html<String> {
    decide_htmx(hx_boosted, "settings")
}

fn decide_htmx(htmx: bool, template: &'static str) ->  Html<String> {
    let template = match htmx {
        true => ENV.get_template(&format!("partials/{}.html", template)),
        false => ENV.get_template(&format!("pages/{}.html", template)),
    };

    // todo: or 500
    let template = template.unwrap();
    Html(template.render(context! {}).unwrap())
}
