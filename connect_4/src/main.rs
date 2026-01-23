use wasmtime::component::*;
use wasmtime::{Engine, Store};

use crate::exports::games::connect::next_move::{Board,};


bindgen!({
    path: "wit/connect.wit",
});

fn main() -> wasmtime::Result<()> {
    let engine = Engine::default();
    let component = Component::from_file(&engine, "target/wasm32-wasip2/release/connect_4_bot.wasm")?;

    let linker = Linker::new(&engine);
    let mut store = Store::new(&engine, ());

    let bindings = Connect::instantiate(&mut store, &component, &linker)?;

    let board = Board {
        heights: vec![ 7, 0, 0, 0, 0, 0, 0 ],
        columns: vec![ 0, 0, 0, 0, 0, 0, 0 ],
    };

    let result = bindings.games_connect_next_move().call_make_move(&mut store, &board)?;

    println!("move made: {}", result);
    Ok(())
}
