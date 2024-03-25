use anyhow::{Error, Result};
use array2d::Array2D;
use crossterm::{
    event::{
        read, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use rand::{seq::SliceRandom, thread_rng};
use ratatui::{
    backend::CrosstermBackend,
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use std::{
    io::{stdout, Stdout},
    sync::mpsc::{channel, RecvError, TryRecvError},
    thread::{sleep, spawn},
    time::{Duration, Instant},
};

#[derive(Debug)]
struct RenderInput<'a>(Paragraph<'a>);
impl<'a> From<&Array2D<(bool, bool)>> for RenderInput<'a> {
    fn from(grid: &Array2D<(bool, bool)>) -> Self {
        Self(
            Paragraph::new(
                grid.rows_iter()
                    .map(|v| {
                        v.map(|&(b, _)| {
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

struct Terminal(ratatui::Terminal<CrosstermBackend<Stdout>>);
impl Terminal {
    fn init() -> Result<Self> {
        enable_raw_mode()?;
        execute!(stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        Ok(Self(ratatui::Terminal::new(CrosstermBackend::new(
            stdout(),
        ))?))
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture).unwrap();
        disable_raw_mode().unwrap();
    }
}

#[derive(Debug)]
enum Signal {
    Click(usize, usize),
    Moved(usize, usize),
    Resize(usize, usize),
    Break,
}

fn main() -> Result<()> {
    let (event_tx, event_rx) = channel::<Signal>();
    let (render_tx, render_rx) = channel::<RenderInput>();

    spawn(move || -> Result<()> {
        loop {
            match read()? {
                Event::Key(KeyEvent {
                    kind: KeyEventKind::Press,
                    code: KeyCode::Char('c'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                }) => event_tx.send(Signal::Break)?,
                Event::Mouse(MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    column,
                    row,
                    ..
                }) => event_tx.send(Signal::Click(row.into(), column.into()))?,
                Event::Mouse(MouseEvent {
                    kind: MouseEventKind::Moved,
                    column,
                    row,
                    ..
                }) => event_tx.send(Signal::Moved(row.into(), column.into()))?,
                Event::Resize(x, y) => event_tx.send(Signal::Resize(x.into(), y.into()))?,
                _ => continue,
            };
        }
    });

    let mut terminal = Terminal::init()?;
    let mut grid = Array2D::filled_with(
        (false, false),
        terminal.0.size()?.height.into(),
        terminal.0.size()?.width.into(),
    );

    spawn(move || -> Result<()> {
        let mut sand_spawner = None::<(usize, usize)>;
        let mut rng = thread_rng();
        Ok('main: loop {
            let clk_start = Instant::now();
            let mut events = event_rx.try_iter();
            while let Some(event) = events.next() {
                match event {
                    Signal::Click(row, col) => {
                        sand_spawner = if sand_spawner.is_none() {
                            Some((row, col))
                        } else {
                            None
                        }
                    }
                    Signal::Moved(row, col) => {
                        sand_spawner = sand_spawner.map(|_| (row, col));
                    }
                    Signal::Resize(_, _) => todo!(),
                    Signal::Break => break 'main,
                }
            }
            for row in 0..grid.column_len() {
                for col in 0..grid.row_len() {
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
                        (Some((false, false)), ..) if sand_spawner == Some((row, col)) => {
                            grid[(row, col)] = (true, true);
                        }
                        (Some((true, false)), Some((false, _)), ..)
                            if row < grid.column_len() - 3 =>
                        {
                            grid[(row, col)] = (false, true);
                            grid[(row + 1, col)] = (true, true);
                        }
                        (Some((true, _)), Some((true, _)), Some((false, _)), Some((false, _)))
                            if row < grid.column_len() - 3 =>
                        {
                            grid[(row, col)] = (false, true);
                            grid[(row + 1, *[col - 1, col + 1].choose(&mut rng).unwrap())] =
                                (true, true);
                        }
                        (Some((true, _)), Some((true, _)), Some((true, _)), Some((false, _)))
                            if row < grid.column_len() - 3 =>
                        {
                            grid[(row, col)] = (false, true);
                            grid[(row + 1, col + 1)] = (true, true);
                        }
                        (Some((true, _)), Some((true, _)), Some((false, _)), Some((true, _)))
                            if row < grid.column_len() - 3 =>
                        {
                            grid[(row, col)] = (false, true);
                            grid[(row + 1, col - 1)] = (true, true);
                        }
                        _ => (),
                    }
                }
            }
            for row in 0..grid.column_len() {
                for col in 0..grid.row_len() {
                    grid[(row, col)].1 = false;
                }
            }
            let delta = Duration::from_millis(16).checked_sub(clk_start.elapsed());
            delta.and_then(|d| Some(sleep(d)));
            render_tx.send((&grid).into())?;
        })
    });

    loop {
        let Ok(widget) = render_rx.recv() else {
            break Ok(());
        };
        terminal
            .0
            .draw(|frame| frame.render_widget(widget.0, frame.size()))?;
    }
}
