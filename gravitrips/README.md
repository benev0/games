# Gravitrips

A Game of falling pieces played on with 7 columns of 6 height. Place 4 in a row, column, diagonal to win.

## Bot Instructions
- To build a bot satisfy the [spec](../wit/gravitrips.wit).
- 1s always to play.
- Output the column to play your next piece 0 indexing.
- Invalid play / crashing will result in a loss

### Board data format
- The board contains a list of heights: the number of pieces played in a column.
- The board also contains column data low order bits are the bottom of a column.
- Data over the height of a column can be ignored.
