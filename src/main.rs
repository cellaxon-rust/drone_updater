extern crate serialport;
mod updater;
mod ui;

use std::{
    error::Error,
    io,
    io::stdout,
    thread,
    time::{Duration, Instant},
};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use tui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};


fn main() -> Result<(), Box<dyn Error>>
{
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let updater = updater::Updater::new();
    let res = run_app(&mut terminal, updater);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}


fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut updater: updater::Updater) -> io::Result<()>
{
    loop
    {
        thread::sleep(Duration::from_millis(1));

        updater.run();

        terminal.draw(|f| ui::ui(f, &updater))?;

        // 키 입력 처리
        let last_tick = Instant::now();
        let tick_rate = Duration::from_millis(1);
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        if event::poll(timeout)?
        {
            if let Event::Key(key) = event::read()?
            {
                match key.code
                {
                    KeyCode::Esc => { return Ok(()); }
                    _ => {}
                }
            }
        }
    }
}
