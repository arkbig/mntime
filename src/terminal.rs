// Copyright © ArkBig
//! This file provides the wrapping function for differences in standard input/output variations.

use crossterm::QueueableCommand as _;
use std::io::Write as _;

pub struct Wrapper<B>
where
    B: tui::backend::Backend,
{
    terminal: Box<tui::Terminal<B>>,

    is_in_tty: bool,
    is_out_tty: bool,
    is_err_tty: bool,
}

impl<B> Wrapper<B>
where
    B: tui::backend::Backend,
{
    pub fn new(backend: B) -> Self {
        match tui::Terminal::new(backend) {
            Err(e) => {
                println!("{:#?}", e);
                panic!();
            }
            Ok(t) => Wrapper {
                terminal: Box::new(t),
                is_in_tty: atty::is(atty::Stream::Stdin),
                is_out_tty: atty::is(atty::Stream::Stdout),
                is_err_tty: atty::is(atty::Stream::Stderr),
            },
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

    pub fn set_cursor(&mut self, x: u16, y: u16) {
        if self.is_out_tty {
            self.terminal.set_cursor(x, y).unwrap();
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

    pub fn clear_after(&mut self) {
        self.flush(true);
        let cur_y = self.get_cursor().1;
        self.draw_if_tty(|f| {
            let size = f.size();
            if cur_y < size.height {
                let rect = tui::layout::Rect::new(0, cur_y, size.width, size.height - cur_y);
                f.render_widget(tui::widgets::Clear, rect);
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
