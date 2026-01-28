use crate::bindings::exports::games::gravitrips::next_move::Board;

mod bindings {
    wit_bindgen::generate!({
        path: "../../wit/gravitrips.wit",
    });

    use super::GravitripsComponent;
    export!(GravitripsComponent);
}

struct GravitripsComponent;

impl bindings::exports::games::gravitrips::next_move::Guest for GravitripsComponent {
    // move in the first open column
    fn make_move(game_state:Board,) -> u8 {
        game_state.heights.iter()
            .enumerate()
            .find(|&(_, &v)| v < 6)
            .and_then(|(k, _)| k.try_into().ok())
            .unwrap_or(7)
    }
}
