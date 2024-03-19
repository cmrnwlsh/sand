use anyhow::{Error, Result};
use array2d::Array2D;
use crossterm::{
    event::{
        read, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
        MouseButton, MouseEvent, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use std::{
    io::stdout,
    sync::mpsc::{channel, TryRecvError},
};
use tokio::task::spawn_blocking;

struct RenderInput<'a>(Paragraph<'a>);
impl<'a> From<&Array2D<bool>> for RenderInput<'a> {
    fn from(grid: &Array2D<bool>) -> Self {
        Self(
            Paragraph::new(
                grid.rows_iter()
                    .map(|v| {
                        v.map(|&b| {
                            Span::styled(
                                " ",
                                if b {
                                    Style::new().on_light_yellow()
                                } else {
                                    Style::default()
                                },
                            )
                        })
                        .collect::<Vec<_>>()
                        .into()
                    })
                    .collect::<Vec<Line>>(),
            )
            .block(Block::default().title("Falling Sand").borders(Borders::ALL)),
        )
    }
}

enum Signal {
    Click(usize, usize),
    Resize(usize, usize),
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let mut grid = Array2D::filled_with(
        false,
        (terminal.size()?.height - 2).into(),
        (terminal.size()?.width - 2).into(),
    );
    let (physics_tx, physics_rx) = channel::<Signal>();

    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen, EnableMouseCapture)?;

    spawn_blocking(move || -> Result<()> {
        loop {
            match read()? {
                Event::Key(KeyEvent {
                    code: KeyCode::Char('c'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                }) => todo!(),
                Event::Mouse(MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    column,
                    row,
                    ..
                }) => physics_tx.send(Signal::Click(row.into(), column.into()))?,
                Event::Resize(_, _) => (),
                _ => (),
            }
        }
    });

    loop {
        match physics_rx.try_recv() {
            Err(TryRecvError::Disconnected) => break,
            Ok(Signal::Click(row, col)) => {
                if let Some(b) = grid.get_mut(row, col) {
                    *b = true;
                }
            }
            _ => (),
        }
        for col in 0..grid.row_len() {
            for row in 0..grid.column_len() {
                let (curr, down, left, right) = (
                    grid.get(row, col).copied(),
                    grid.get(row + 1, col).copied(),
                    if col > 0 {
                        grid.get(row + 1, col - 1).copied()
                    } else {
                        None
                    },
                    grid.get(row + 1, col + 1).copied(),
                );
                match (curr, down, left, right) {
                    (Some(true), Some(false), _, _) => {
                        *grid.get_mut(row, col).unwrap() = false;
                        *grid.get_mut(row + 1, col).unwrap() = true;
                        break;
                    }
                    (Some(true), Some(true), Some(true), Some(false)) => {
                        *grid.get_mut(row, col).unwrap() = false;
                        *grid.get_mut(row + 1, col + 1).unwrap() = true;
                    }
                    (Some(true), Some(true), Some(false), Some(true)) => {
                        *grid.get_mut(row, col).unwrap() = false;
                        *grid.get_mut(row + 1, col - 1).unwrap() = true;
                    }
                    _ => (),
                }
            }
        }
        terminal.draw(|frame| frame.render_widget(RenderInput::from(&grid).0, frame.size()))?;
    }

    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture).map_err(Error::from)
}
