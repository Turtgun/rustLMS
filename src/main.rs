use glib::clone::Downgrade;
use glib::property::PropertyGet;
use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Box as GtkBox, Button, Entry, Label, ListStore, ScrolledWindow, TreeView, TreeViewColumn, CellRendererText};
use std::ops::Deref;
use std::rc::Rc;
use std::cell::RefCell;
use std::fs::File;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use csv;
use chrono::{self, DateTime, Months, Utc};
use serde::{Deserialize, Serialize};


#[derive(Clone)]
struct LiItemInstance {
    title: String,
    id: u32,
    renew_factor: u32,
    due_date: DateTime<Utc>,
    notice: bool
}

impl LiItemInstance {
    fn renew(&mut self) {
        self.due_date = self.due_date + Months::new(1*self.renew_factor);
    }
}

#[derive(Clone, Deserialize, Serialize)]
struct LiItem {
    title: String,
    author: Option<Box<String>>,
    year: u32,
    edition: String,
    desc: String,
    format: String,
    id: u32,
    copies: u32,
    avail_copies: u32,
    ratings: u32,
}

impl LiItem {
    fn create_instance(&mut self) -> LiItemInstance {
        self.avail_copies -= 1;

        let mut inst = LiItemInstance{
            title: self.title.clone(),
            id: self.id,
            renew_factor: match self.format.to_lowercase().as_str() {
                "book" => 1,
                "movie" => 2,
                _ => 0
            },
            due_date: DateTime::default(),
            notice: false
        };

        inst.renew();

        return inst;
    }
}

#[derive(Clone)]
struct Member {
    id: u32,
    items: HashMap<u32, LiItemInstance>,
}

struct Library {
    items: HashMap<u32, LiItem>,
    members: HashMap<u32, Member>,
}

impl Library {
    fn new() -> Library {
        Library {
            items: HashMap::with_capacity(3000000),
            members: HashMap::with_capacity(10),
        }
    }

    fn initialize_lib(&mut self, csv_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let file = File::open(csv_path).map_err(|e| {
            eprintln!("Failed to open file: {}", e);
            println!("Attempted to open file: {}", csv_path);
            e
        })?;
        let mut rdr = csv::Reader::from_reader(file);
        let mut count = 0;
        for (i, result) in rdr.deserialize().enumerate() {
            match result {
                Ok(item) => {
                    let item: LiItem = item;
                    self.items.insert(item.id, item);
                    count += 1;
                }
                Err(e) => {
                    eprintln!("Failed to parse row {}: {}", i + 2, e);
                    // Print details about the row
                    println!("Failed row details: {:?}", e);
                }
            }
        }
        println!("Loaded {} items into library", count);
        if count == 0 {
            return Err("No items loaded from CSV".into());
        }
        Ok(())
    }
    
    fn book_issue(&mut self, title_id: u32, member_id_text: String) -> Result<(), String> {
        if let Ok(member_id) = member_id_text.parse::<u32>() {
            if let Some(member) = self.members.get_mut(&member_id) {
                if let Some(item) = self.items.get_mut(&title_id) {
                    if item.avail_copies > 0 {
                        member.items.insert(title_id, item.create_instance());
                        Ok(())
                    } else {
                        Err("No available copies left!".to_string())
                    }
                } else {
                    Err("Invalid Item ID!".to_string())
                }
            } else {
                Err("Invalid Member ID!".to_string())
            }
        } else {
            let member_id = self.members.len() as u32 + 1;
            if let Some(item) = self.items.get_mut(&title_id) {
                if item.avail_copies > 0 {
                    let mut member = Member {
                        id: member_id,
                        items: HashMap::new(),
                    };

                    member.items.insert(title_id, item.create_instance());

                    self.members.insert(member_id, member);
                    Ok(())
                } else {
                    Err("No available copies left!".to_string())
                }
            } else {
                Err("Invalid Item ID!".to_string())
            }
        }
    }


    fn book_return(&mut self, title_id: u32, member_id: u32) -> Result<&mut LiItem, String>{
        if self.members.contains_key(&member_id) {
            if let Some(inst) = self.members.get_mut(&member_id).unwrap().items.remove(&title_id) {
                if let Some(item) = self.items.get_mut(&title_id) {
                    item.avail_copies += 1;
                    drop(inst);
                    Ok(item)
                } else {
                    Err("Book not found in library items".to_string())
                }
            } else {
                Err("This book was not checked out by this member".to_string())
            }
        } else {
            Err("Member not found".to_string())
        }
    }
}

fn create_library_gui() -> Application {
    let app = Application::builder()
        .application_id("com.example.rustLMS")
        .build();

    app.connect_activate(|app| {
        // Shared library state
        let library = Arc::new(RwLock::new(Library::new()));

        // Initialize library
        {
            let mut lib = library.write().unwrap();
            match lib.initialize_lib("output.csv") {
                Ok(_) => println!("Library initialized successfully"),
                Err(e) => {
                    eprintln!("Failed to initialize library: {}", e);
                    if let Ok(path) = std::env::current_dir() {
                        println!("Current working directory: {}", path.display());
                    }
                }
            }
        }

        let window = ApplicationWindow::builder()
            .application(app)
            .title("Library Management System")
            .default_width(800)
            .default_height(600)
            .build();

        let main_box = GtkBox::new(gtk::Orientation::Vertical, 10);
        main_box.set_margin_top(10);
        main_box.set_margin_bottom(10);
        main_box.set_margin_start(10);
        main_box.set_margin_end(10);

        let notebook = gtk::Notebook::new();

        // Pass the Arc<RwLock<Library>> to each page
        notebook.append_page(
            &create_issue_page(library.clone()),
            Some(&Label::new(Some("Issue Books"))),
        );
        notebook.append_page(
            &create_return_page(library.clone()),
            Some(&Label::new(Some("Return Books"))),
        );
        notebook.append_page(
            &create_member_details_page(library.clone()),
            Some(&Label::new(Some("Member Details"))),
        );
        notebook.append_page(
            &create_catalog_page(library.clone()),
            Some(&Label::new(Some("Library Catalog"))),
        );

        main_box.append(&notebook);
        window.set_child(Some(&main_box));
        window.show();
    });

    app
}
fn create_issue_page(library: Arc<RwLock<Library>>) -> GtkBox {
    let issue_box = GtkBox::new(gtk::Orientation::Vertical, 10);

    let item_id_label = Label::new(Some("Item ID:"));
    let item_id_entry = Entry::new();
    let member_id_label = Label::new(Some("Member ID:"));
    let member_id_entry = Entry::new();
    let status_label = Label::new(None);

    let issue_button = Button::with_label("Issue Book");
    issue_button.connect_clicked(glib::clone!(
        #[weak] item_id_entry,
        #[weak] member_id_entry,
        #[weak] status_label,
        #[strong] library,  // Keep the Arc alive
        move |_| {
            let item_id_text = item_id_entry.text().to_string();
            let member_id_text = member_id_entry.text().to_string();
            if let Ok(item_id) = item_id_text.parse::<u32>() {
                let mut lib = library.write().unwrap(); // Lock for writing here
                match lib.book_issue(item_id, member_id_text) {
                    Ok(_) => {
                        status_label.set_text("Book issued successfully!");
                        item_id_entry.set_text("");
                        member_id_entry.set_text("");
                    }
                    Err(e) => status_label.set_text(&format!("Error: {}", e)),
                }
            } else {
                status_label.set_text("Invalid Item ID");
            }
        }
    ));

    issue_box.append(&item_id_label);
    issue_box.append(&item_id_entry);
    issue_box.append(&member_id_label);
    issue_box.append(&member_id_entry);
    issue_box.append(&issue_button);
    issue_box.append(&status_label);

    issue_box
}

fn create_return_page(library: Arc<RwLock<Library>>) -> GtkBox {
    let return_box = GtkBox::new(gtk::Orientation::Vertical, 10);

    let item_id_label = Label::new(Some("Item ID:"));
    let item_id_entry = Entry::new();
    let member_id_label = Label::new(Some("Member ID:"));
    let member_id_entry = Entry::new();
    let status_label = Label::new(None);
    let book_details_label = Label::new(None);

    let return_button = Button::with_label("Return Book");
    return_button.connect_clicked(glib::clone!(
        #[weak] item_id_entry,
        #[weak] member_id_entry,
        #[weak] status_label,
        #[weak] book_details_label,
        #[strong] library,
        move |_| {
            let item_id_text = item_id_entry.text().to_string();
            let member_id_text = member_id_entry.text().to_string();

            if let Ok(item_id) = item_id_text.parse::<u32>() {
                if let Ok(member_id) = member_id_text.parse::<u32>() {
                    let mut lib = library.write().unwrap(); // Lock for writing
                    match lib.book_return(item_id, member_id) {
                        Ok(book) => {
                            status_label.set_text("Book returned successfully!");
                            book_details_label.set_text(&format!(
                                "Returned Book: {} (ID: {})",
                                book.title, book.id
                            ));
                            item_id_entry.set_text("");
                            member_id_entry.set_text("");
                        }
                        Err(e) => {
                            status_label.set_text(&format!("Error: {}", e));
                            book_details_label.set_text("");
                        }
                    }
                } else {
                    status_label.set_text("Invalid Member ID");
                }
            } else {
                status_label.set_text("Invalid Item ID");
            }
        }
    ));

    return_box.append(&item_id_label);
    return_box.append(&item_id_entry);
    return_box.append(&member_id_label);
    return_box.append(&member_id_entry);
    return_box.append(&return_button);
    return_box.append(&status_label);
    return_box.append(&book_details_label);

    return_box
}

fn create_member_details_page(library: Arc<RwLock<Library>>) -> GtkBox {
    let member_box = GtkBox::new(gtk::Orientation::Vertical, 10);
    
    // Create a list store for members
    let list_store = ListStore::new(&[
        u32::static_type(),     // Member ID
        String::static_type(),  // Book Title
    ]);
    
    // Create TreeView
    let tree_view = TreeView::with_model(&list_store);
    // Create columns
    let columns = [
        ("Member ID", 0),
        ("Item Titles", 1),
    ];
    
    for (title, column_id) in columns.iter() {
        let renderer = CellRendererText::new();
        let column = TreeViewColumn::new();
        column.set_title(title);
        column.pack_start(&renderer, true);
        column.add_attribute(&renderer, "text", *column_id);
        tree_view.append_column(&column);
    }
    
    // Refresh Button
    let refresh_button = Button::with_label("Refresh Members");
    refresh_button.connect_clicked(glib::clone!(
        #[weak]
        list_store,
        #[weak]
        library,
        move |_| {
            list_store.clear();
            for member in library.read().unwrap().members.values() {
                let mut titles = String::new();
                for inst in member.items.values() {
                    titles += &(inst.title.as_str().to_owned() + " (" + &inst.id.to_string() +  "),  ");
                }
                list_store.insert_with_values(None, &[
                    (0, &member.id),
                    (1, &titles),
                ]);
            }
        }
    ));
    
    // Scrolled Window for TreeView
    let scrolled_window = ScrolledWindow::new();
    scrolled_window.set_child(Some(&tree_view));
    scrolled_window.set_vexpand(true);
    
    // Add widgets to box
    member_box.append(&refresh_button);
    member_box.append(&scrolled_window);
    
    member_box
}
fn create_catalog_page(library: Arc<RwLock<Library>>) -> GtkBox {
    let catalog_box = GtkBox::new(gtk::Orientation::Vertical, 10);
    
    let list_store = ListStore::new(&[
        u32::static_type(),     // Item ID
        String::static_type(),  // Title
        String::static_type(),  // Author
        u32::static_type(),     // Year
        String::static_type(),  // Format
        u32::static_type(),     // Total Copies
        u32::static_type(),     // Available Copies
        u32::static_type(),     // Ratings
    ]);
    
    let tree_view = TreeView::with_model(&list_store);

    let columns = [
        ("Item ID", 0),
        ("Title", 1),
        ("Author", 2),
        ("Year", 3),
        ("Format", 4),
        ("Total Copies", 5),
        ("Available Copies", 6),
        ("Ratings", 7)
    ];

    for (title, column_id) in columns.iter() {
        let renderer = CellRendererText::new();
        let column = TreeViewColumn::new();
        column.set_title(title);
        column.pack_start(&renderer, true);
        column.add_attribute(&renderer, "text", *column_id);
        tree_view.append_column(&column);
    }

    let refresh_button = Button::with_label("Refresh Catalog");
    
    let refresh_catalog = |list_store: &ListStore, library: &Library| {
        list_store.clear();
        for (_, item) in &library.items {
            list_store.insert_with_values(None, &[
                (0, &item.id),
                (1, &item.title),
                (2, &item.author.as_ref().map_or("Unknown".to_string(), |a| a.to_string())),
                (3, &item.year),
                (4, &item.format),
                (5, &item.copies),
                (6, &item.avail_copies),
                (7, &item.ratings),
            ]);
        }
    };

    // Populate catalog on startup
    {
        refresh_catalog(&list_store, &library.read().unwrap());
    }


    refresh_button.connect_clicked(glib::clone!(
        #[weak]
        list_store,
        #[strong]
        library,
        move |_| {
            refresh_catalog(&list_store, &library.read().unwrap());
        }
    ));

    let scrolled_window = ScrolledWindow::new();
    scrolled_window.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
    scrolled_window.set_child(Some(&tree_view));
    scrolled_window.set_vexpand(true);

    catalog_box.append(&refresh_button);
    catalog_box.append(&scrolled_window);

    catalog_box
}

fn main() {
    let app = create_library_gui();
    app.run();
}