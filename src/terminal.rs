// Copyright © ArkBig
//! This file provides the wrapping function for differences in standard input/output variations.

use crossterm::QueueableCommand as _;
use std::io::Write as _;

pub struct Wrapper<B>
where
    B: ratatui::backend::Backend,
{
    terminal: Option<Box<ratatui::Terminal<B>>>,

    is_in_tty: bool,
    is_out_tty: bool,
    is_err_tty: bool,
}

impl<B> Wrapper<B>
where
    B: ratatui::backend::Backend,
{
    pub fn new(backend: B) -> Self {
        let is_in_tty = atty::is(atty::Stream::Stdin);
        let is_out_tty = atty::is(atty::Stream::Stdout);
        let is_err_tty = atty::is(atty::Stream::Stderr);
        let terminal = if is_in_tty && is_out_tty {
            if let Ok(t) = ratatui::Terminal::new(backend) {
                Some(Box::new(t))
            } else {
                None
            }
        } else {
            None
        };
        Wrapper {
            terminal,
            is_in_tty,
            is_out_tty,
            is_err_tty,
        }
    }

    pub fn terminal_mut(&mut self) -> Option<&mut ratatui::Terminal<B>> {
        match &mut self.terminal {
            Some(t) => Some(t.as_mut()),
            None => None,
        }
    }

    pub fn get_cursor(&mut self) -> (u16, u16) {
        let is_in_tty = self.is_in_tty;
        let is_out_tty = self.is_out_tty;
        match self.terminal_mut() {
            Some(t) => {
                if is_in_tty && is_out_tty {
                    t.get_cursor().unwrap()
                } else {
                    (0, t.size().unwrap().y)
                }
            }
            None => (0, 0),
        }
    }

    pub fn set_cursor(&mut self, x: u16, y: u16) {
        if self.is_out_tty && self.terminal.is_some() {
            self.terminal_mut().unwrap().set_cursor(x, y).unwrap();
        }
    }

    pub fn draw_if_tty<F>(&mut self, f: F)
    where
        F: FnOnce(&mut ratatui::Frame),
    {
        if self.is_out_tty && self.terminal.is_some() {
            self.terminal_mut().unwrap().draw(f).unwrap();
        }
    }

    pub fn clear_after(&mut self) {
        self.flush(true);
        let cur_y = self.get_cursor().1;
        self.draw_if_tty(|f| {
            let size = f.size();
            if cur_y < size.height {
                let rect = ratatui::layout::Rect::new(0, cur_y, size.width, size.height - cur_y);
                f.render_widget(ratatui::widgets::Clear, rect);
            }
        });
        self.set_cursor(0, cur_y);
    }

    pub fn queue_attribute(&self, attr: crossterm::style::Attribute) {
        if self.is_out_tty {
            std::io::stdout()
                .queue(crossterm::style::SetAttribute(attr))
                .unwrap();
        }
    }
    pub fn queue_attribute_err(&self, attr: crossterm::style::Attribute) {
        if self.is_out_tty {
            std::io::stderr()
                .queue(crossterm::style::SetAttribute(attr))
                .unwrap();
        }
    }
    pub fn queue_fg(&self, color: crossterm::style::Color) {
        if self.is_out_tty {
            std::io::stdout()
                .queue(crossterm::style::SetForegroundColor(color))
                .unwrap();
        }
    }
    pub fn queue_fg_err(&self, color: crossterm::style::Color) {
        if self.is_out_tty {
            std::io::stderr()
                .queue(crossterm::style::SetForegroundColor(color))
                .unwrap();
        }
    }
    pub fn queue_print<T>(&self, print: crossterm::style::Print<T>)
    where
        T: std::fmt::Display,
    {
        std::io::stdout().queue(print).unwrap();
    }
    pub fn queue_print_err<T>(&self, print: crossterm::style::Print<T>)
    where
        T: std::fmt::Display,
    {
        std::io::stderr().queue(print).unwrap();
    }
    pub fn flush(&self, reset: bool) {
        if reset && self.is_out_tty {
            std::io::stdout()
                .queue(crossterm::style::SetAttribute(
                    crossterm::style::Attribute::Reset,
                ))
                .unwrap();
        }
        std::io::stdout().flush().unwrap();
    }
    pub fn flush_err(&self, reset: bool) {
        if reset && self.is_err_tty {
            std::io::stderr()
                .queue(crossterm::style::SetAttribute(
                    crossterm::style::Attribute::Reset,
                ))
                .unwrap();
        }
        std::io::stderr().flush().unwrap();
    }
}
