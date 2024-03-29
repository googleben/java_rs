use gdk_pixbuf::{Colorspace, Pixbuf};
use self::glib::Bytes;
use gtk::prelude::{BoxExt, ButtonExt, ContainerExt, FileChooserExt, GtkMenuItemExt, GtkWindowExt, MenuShellExt, NotebookExt, NotebookExtManual, WidgetExt};
use self::glib::IsA;
use gtk::*;
use gtk;
use icon::ICON;
use inner::class_to_tree;
use java_class::class::JavaClass;
use java_class::cp_info::CPInfo;
use std::rc::Rc;
use std::str;
use std::sync::Mutex;

pub fn make_gui() {
    if gtk::init().is_err() {
        println!("Failed to initialize GTK.");
        return;
    }

    let window = Window::new(WindowType::Toplevel);
    window.set_title("Class Browser");
    window.set_default_size(500, 700);

    let filem = MenuItem::with_label("File");
    let v_box = gtk::Box::new(gtk::Orientation::Vertical, 10);
    window.add(&v_box);

    let menu = Menu::new();
    let open = MenuItem::with_label("Open");

    menu.append(&open);
    filem.set_submenu(Some(&menu));

    let menu_bar = MenuBar::new();
    menu_bar.append(&filem);
    v_box.pack_start(&menu_bar, false, false, 0);

    let nb = setup_notebook(open);
    nb.notebook.set_scrollable(true);

    v_box.pack_end(&nb.notebook, true, true, 0);

    let mut v = Vec::<u8>::with_capacity(16384);
    for i in ICON.iter() {
        v.push(*i);
    }

    let icon_buf = Pixbuf::from_bytes(&Bytes::from(&v), Colorspace::Rgb, true, 8, 64, 64, 64 * 4);

    window.set_icon(Some(&icon_buf));

    window.show_all();

    window.connect_delete_event(|_, _| {
        gtk::main_quit();
        Inhibit(false)
    });

    gtk::main();
}

fn setup_notebook(open: MenuItem) -> Rc<CNotebook> {
    let notebook = Rc::new(CNotebook::new());
    let last_dir_base: Rc<Mutex<Option<String>>> = Rc::new(Mutex::new(None));
    let notebook_clone = Rc::clone(&notebook);
    let last_dir = Rc::clone(&last_dir_base);
    open.connect_activate(move |_| {
        let fchoose = FileChooserDialog::new::<Window>(Some("Open"), None, FileChooserAction::Open);
        let ldir2 = Rc::clone(&last_dir);
        let guard = ldir2.lock().unwrap();
        if let Some(ref s) = *guard { 
            fchoose.set_current_folder_uri(s); 
        };
        drop(guard);
        let notebook = Rc::clone(&notebook_clone);
        let last_dir = Rc::clone(&last_dir);
        fchoose.connect_file_activated(move |x| {
            let file = x.filename();
            let folder = x.current_folder_uri();
            if let Some(folder) = folder {
                let mut p = last_dir.lock().unwrap();
                *p = Some(folder.as_str().to_owned());
            }
            x.close();
            match file {
                None => {}
                Some(f) => {
                    let p = f.as_path();
                    let scrollw = ScrolledWindow::builder().build();
                    let class = match JavaClass::new(p.to_str().unwrap()) {
                        Ok(a) => a,
                        Err(e) => {
                            println!("{}", e);
                            return;
                        }
                    };
                    let name = match class.constant_pool[class.this_class] {
                        CPInfo::Class { name_index } => {
                            match &class.constant_pool[name_index] {
                                CPInfo::Utf8 { bytes, .. } => {
                                    str::from_utf8(&bytes).unwrap().to_owned()
                                }
                                _ => "Class Pool index did not point to Utf8".to_owned()
                            }
                        }
                        _ => "Class Pool index did not point to Utf8".to_owned()
                    };
                    scrollw.add(&class_to_tree(class));
                    notebook.create_tab(&name, scrollw);
                }
            }
        });
        fchoose.show_all();
    });
    Rc::clone(&notebook)
}

struct CNotebook {
    notebook: gtk::Notebook
}

impl CNotebook {
    fn new() -> CNotebook {
        CNotebook {
            notebook: gtk::Notebook::new()
        }
    }

    fn create_tab<T: IsA<Widget> + 'static>(&self, title: &str, widget: T) -> u32 {
        let close_image = gtk::Image::from_icon_name(Some("window-close"),
                                                         IconSize::Button.into());
        let button = gtk::Button::new();
        let label = gtk::Label::new(Some(title));
        let tab = gtk::Box::new(Orientation::Horizontal, 0);

        button.set_relief(ReliefStyle::None);
        button.set_focus_on_click(false);
        button.add(&close_image);

        tab.pack_start(&label, false, false, 0);
        tab.pack_start(&button, false, false, 0);
        tab.show_all();

        let index = self.notebook.append_page(&widget, Some(&tab));
        self.notebook.set_tab_reorderable(&widget, true);

        let notebook_clone = self.notebook.clone();
        button.connect_clicked(move |_| {
            let index = notebook_clone.page_num(&widget)
                .expect("Couldn't get page_num from notebook_clone");
            notebook_clone.remove_page(Some(index));
        });

        self.notebook.show_all();

        index
    }
}