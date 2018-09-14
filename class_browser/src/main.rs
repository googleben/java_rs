#![windows_subsystem = "windows"]

extern crate gtk;
extern crate gdk_pixbuf;
extern crate java_class;

mod gui;
mod icon;
mod inner;

fn main() {
    gui::make_gui();
}
