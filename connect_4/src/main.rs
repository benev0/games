use wasmtime::component::*;
use wasmtime::{Engine, Store};

use crate::exports::games::connect::next_move::{Board,};


bindgen!({
    path: "wit/connect.wit",
});

const BOARD_ROWS: u8 = 6;
const BOARD_COLUMNS: u8 = 7;
const WIN_CONNECT: u8 = 4;
const VERTICAL_WIN: u8 = 15;

#[derive(Debug)]
enum BadMove {
    ColumnInvalid,
    ColumnFull,
    WasmError
}

#[derive(Debug)]
enum GameEnd{
    Win(bool),
    Loss(bool, BadMove),
    Draw,
}

impl Board {
    // checks if topmost pice is part of a 4 zero chain
    fn check_win(&self, column: u8) -> bool {
        // vertical
        if let Some(&column_data) = self.columns.get::<usize>(column.into())
            && let Some(&height_data) = self.heights.get::<usize>(column.into())
            && height_data >= WIN_CONNECT
            && !(column_data >> (height_data - WIN_CONNECT)) & VERTICAL_WIN == VERTICAL_WIN {
            return true;
        }

        // horasantal
        if let Some(&column_data) = self.columns.get::<usize>(column.into())
            && let Some(&height_level) = self.heights.get::<usize>(column.into())
            && (column_data >> (height_level - 1)) & 1 == 0
        {

            let mut seq_count = 1;

            let mut offset = 1;
            // right side
            while let Some(&height_adjacent) = self.heights.get::<usize>(<u8 as Into<usize>>::into(column) + offset)
                && height_adjacent >= height_level
                && let Some(&col_adjacent) = self.columns.get::<usize>(<u8 as Into<usize>>::into(column) + offset)
                && (col_adjacent >> (height_level - 1)) & 1 == 0 {
                seq_count += 1;
                offset += 1
            }

            offset = 1;
            // left side
            while let Some(&height_adjacent) = self.heights.get::<usize>(<u8 as Into<usize>>::into(column) - offset)
                && height_adjacent >= height_level {
                seq_count += 1;
                offset += 1
            }


            if seq_count >= WIN_CONNECT {
                return true;
            }
        }

        // diagonal forward
        if let Some(&column_data) = self.columns.get::<usize>(column.into())
            && let Some(&height_level) = self.heights.get::<usize>(column.into())
            && (column_data >> (height_level - 1)) & 1 == 0
        {

            let mut seq_count = 1;

            let mut offset = 1;
            // right side
            while let Some(&height_adjacent) = self.heights.get::<usize>(<u8 as Into<usize>>::into(column) + offset) {
                if let Ok(offset_u8) = <usize as TryInto<u8>>::try_into(offset)
                    && height_adjacent >= height_level + offset_u8
                    && let Some(&col_adjacent) = self.columns.get::<usize>(<u8 as Into<usize>>::into(column) + offset)
                    && (col_adjacent >> (height_level - 1 - offset_u8)) & 1 == 0
                {
                    seq_count += 1;
                    offset += 1
                } else {
                    break;
                }
            }

            offset = 1;
            // left side
            while let Some(&height_adjacent) = self.heights.get::<usize>(<u8 as Into<usize>>::into(column) - offset) {
                if let Ok(offset_u8) = <usize as TryInto<u8>>::try_into(offset)
                    && height_adjacent >= height_level - offset_u8
                    && let Some(&col_adjacent) = self.columns.get::<usize>(<u8 as Into<usize>>::into(column) - offset)
                    && (col_adjacent >> (height_level - 1 + offset_u8)) & 1 == 0
                {
                    seq_count += 1;
                    offset += 1
                } else {
                    break;
                }
            }


            if seq_count >= WIN_CONNECT {
                return true;
            }
        }

        // diagonal backward
        if let Some(&column_data) = self.columns.get::<usize>(column.into())
            && let Some(&height_level) = self.heights.get::<usize>(column.into())
            && (column_data >> (height_level - 1)) & 1 == 0
        {

            let mut seq_count = 1;

            let mut offset = 1;
            // right side
            while let Some(&height_adjacent) = self.heights.get::<usize>(<u8 as Into<usize>>::into(column) + offset) {
                if let Ok(offset_u8) = <usize as TryInto<u8>>::try_into(offset)
                    && height_adjacent >= height_level - offset_u8
                    && let Some(&col_adjacent) = self.columns.get::<usize>(<u8 as Into<usize>>::into(column) + offset)
                    && (col_adjacent >> (height_level - 1 + offset_u8)) & 1 == 0
                {
                    seq_count += 1;
                    offset += 1
                } else {
                    break;
                }
            }

            offset = 1;
            // left side
            while let Some(&height_adjacent) = self.heights.get::<usize>(<u8 as Into<usize>>::into(column) - offset) {
                if let Ok(offset_u8) = <usize as TryInto<u8>>::try_into(offset)
                    && height_adjacent >= height_level + offset_u8
                    && let Some(&col_adjacent) = self.columns.get::<usize>(<u8 as Into<usize>>::into(column) - offset)
                    && (col_adjacent >> (height_level - 1 - offset_u8)) & 1 == 0
                {
                    seq_count += 1;
                    offset += 1
                } else {
                    break;
                }
            }


            if seq_count >= WIN_CONNECT {
                return true;
            }
        }

        false
    }

    fn confirm_move(&mut self, column: u8) -> Result<(), BadMove> {

        let height: &mut u8 = self.heights.get_mut::<usize>(column.into()).ok_or(BadMove::ColumnInvalid)?;
        let column: &mut u8 = self.columns.get_mut::<usize>(column.into()).ok_or(BadMove::ColumnInvalid)?;

        if *height >= BOARD_ROWS {
            return Err(BadMove::ColumnFull);
        }

        *column = *column | (1 << *height);

        *height += 1;

        self.columns.iter_mut().for_each(|col| {
            *col = !(*col);
        });

        Ok(())
    }
}

fn main() -> wasmtime::Result<()> {
    let engine = Engine::default();
    let p1_component = Component::from_file(&engine, "target/wasm32-wasip2/release/connect_4_bot.wasm")?;
    let p2_component = Component::from_file(&engine, "target/wasm32-wasip2/release/connect_4_bot.wasm")?;

    let linker = Linker::new(&engine);
    let mut store = Store::new(&engine, ());

    let p1_bindings = Connect::instantiate(&mut store, &p1_component, &linker)?;
    let p2_bindings = Connect::instantiate(&mut store, &p2_component, &linker)?;

    let mut board = Board {
        heights: vec![ 0, 0, 0, 0, 0, 0, 0 ],
        columns: vec![ 0, 0, 0, 0, 0, 0, 0 ],
    };

    // p1: true, p2 false
    let mut move_count = 0;
    let mut to_play = true;
    let win = loop {
        if move_count >= BOARD_ROWS * BOARD_COLUMNS {
            break GameEnd::Draw;
        }

        let move_result = match to_play {
            true => p1_bindings.games_connect_next_move().call_make_move(&mut store, &board),
            false => p2_bindings.games_connect_next_move().call_make_move(&mut store, &board),
        };

        let result = if let Ok(result) = move_result {
            result
        } else {
            break GameEnd::Loss(to_play, BadMove::WasmError)
        };

        if let Err(status) = board.confirm_move(result) {
            break GameEnd::Loss(to_play, status);
        } else if board.check_win(result) {
            break GameEnd::Win(to_play);
        }

        move_count += 1;
        to_play = !to_play;
    };


    dbg!(&board);
    dbg!(&win);
    dbg!(move_count + 1);
    Ok(())
}
