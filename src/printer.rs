use std::fmt::Display;

use colored::{Color, Colorize};

fn print_err<D: Display>(name: &str, msg: D) {
    eprintln!("{}: {}", name.red(), msg);
}

fn print_dbg<C: Into<Color>, D: Display>(name: &str, msg: D, color: C) {
    println!("{}: {}", name.color(color).dimmed(), msg);
}

const WCOV_COLOR: Color = Color::Yellow;
const ANNOTATE_COLOR: Color = Color::BrightBlue;
const RUNNER_COLOR: Color = Color::BrightGreen;

pub fn println_wcov_error<D: Display>(msg: D) {
    print_err("WCOV", msg);
}

pub fn println_wcov_dbg<D: Display>(msg: D) {
    print_dbg("wcov", msg, WCOV_COLOR);
}

pub fn println_annotate_error<D: Display>(msg: D) {
    print_err("Annotator", msg);
}

pub fn println_annotate_dbg<D: Display>(msg: D) {
    print_dbg("Annotator", msg, ANNOTATE_COLOR);
}

pub fn println_runner_error<D: Display>(msg: D) {
    print_err("Runner", msg);
}

pub fn println_runner_dbg<D: Display>(msg: D) {
    print_dbg("Runner", msg, RUNNER_COLOR);
}
