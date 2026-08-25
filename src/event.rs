use crossterm::event::{self, Event as CrosstermEvent, KeyEvent};
use std::io;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
pub enum Event {
    Tick,
    Key(KeyEvent),
    Resize,
}

pub struct EventLoop {
    next_tick: Instant,
    interval: Duration,
}

impl EventLoop {
    pub fn new(interval: Duration) -> Self {
        Self {
            next_tick: Instant::now() + interval,
            interval,
        }
    }
    pub fn set_interval(&mut self, interval: Duration) {
        self.interval = interval;
        self.next_tick = Instant::now() + interval;
    }
    pub fn next(&mut self) -> io::Result<Event> {
        let timeout = self.next_tick.saturating_duration_since(Instant::now());
        if event::poll(timeout)? {
            return match event::read()? {
                CrosstermEvent::Key(key) => Ok(Event::Key(key)),
                CrosstermEvent::Resize(_, _) => Ok(Event::Resize),
                _ => self.next(),
            };
        }
        self.next_tick = Instant::now() + self.interval;
        Ok(Event::Tick)
    }
}
