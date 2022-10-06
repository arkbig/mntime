use std::io::Write;

use crossterm::QueueableCommand;

pub struct Wrapper<B>
where
    B: tui::backend::Backend,
{
    terminal: Box<tui::Terminal<B>>,

    is_in_tty: bool,
    is_out_tty: bool,
    _is_err_tty: bool,
}

impl<B> Wrapper<B>
where
    B: tui::backend::Backend,
{
    pub fn new(backend: B) -> Self {
        Wrapper {
            terminal: Box::new(tui::Terminal::new(backend).unwrap()),
            is_in_tty: atty::is(atty::Stream::Stdin),
            is_out_tty: atty::is(atty::Stream::Stdout),
            _is_err_tty: atty::is(atty::Stream::Stderr),
        }
    }

    pub fn terminal(&self) -> &tui::Terminal<B> {
        self.terminal.as_ref()
    }
    pub fn terminal_mut(&mut self) -> &mut tui::Terminal<B> {
        self.terminal.as_mut()
    }

    pub fn get_cursor(&mut self) -> (u16, u16) {
        if self.is_in_tty && self.is_out_tty {
            self.terminal_mut().get_cursor().unwrap()
        } else {
            (0, self.terminal().size().unwrap().y)
        }
    }

    pub fn draw_if_tty<F>(&mut self, f: F)
    where
        F: FnOnce(&mut tui::Frame<B>),
    {
        if self.is_out_tty {
            self.terminal_mut().draw(f).unwrap();
        }
    }

    pub fn queue_attribute(&self, attr: crossterm::style::SetAttribute) {
        if self.is_out_tty {
            std::io::stdout().queue(attr);
        }
    }
    pub fn queue_print<T>(&self, print: crossterm::style::Print<T>)
    where
        T: std::fmt::Display,
    {
        std::io::stdout().queue(print);
    }
    pub fn flush(&self, reset: bool) {
        if reset {
            std::io::stdout().queue(crossterm::style::SetAttribute(
                crossterm::style::Attribute::Reset,
            ));
        }
        std::io::stdout().flush();
    }
}
