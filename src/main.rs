extern crate gtk;
extern crate gdk;
extern crate glib;
extern crate cairo;
extern crate neovim_lib;
extern crate rmp;

mod nvim;
mod ui_model;
mod ui;

fn main() {
    gtk::init().expect("Failed to initialize GTK");
    ui::UI.with(|ui_cell| {
        let mut ui = ui_cell.borrow_mut();
        ui.init();

        nvim::initialize(&mut *ui).expect("Can't start nvim instance");
    });

    gtk::main();       
}

