use core::time;
use std::{borrow::Cow, fmt::Display};

pub struct Spinner {
    spinner: indicatif::ProgressBar,
}

pub enum Colour {
    Green,
}

impl Spinner {
    pub fn new() -> Self {
        let spinner = indicatif::ProgressBar::new_spinner();
        spinner.set_style(
            indicatif::ProgressStyle::with_template("{spinner:.white} {msg} [{elapsed}]").unwrap(),
        );
        spinner.enable_steady_tick(time::Duration::from_millis(50));
        Spinner { spinner }
    }

    pub fn status(&self, msg: impl Into<Cow<'static, str>>) {
        self.spinner.set_message(msg);
    }

    pub fn print_above<T: AsRef<str> + Display>(&self, msg: T, colour: Colour) {
        self.spinner.suspend(|| {
            println!("{}", get_coloured_message(msg, colour));
        })
    }
}

fn get_coloured_message<T: AsRef<str> + Display>(
    msg: T,
    colour: Colour,
) -> console::StyledObject<T> {
    match colour {
        Colour::Green => console::style(msg).green(),
    }
}
