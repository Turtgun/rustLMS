use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Box as GtkBox, Button, Entry, Label, ListStore, ScrolledWindow, TreeView, TreeViewColumn, CellRendererText};
use std::rc::Rc;
use std::cell::RefCell;
use std::fs::File;
use std::collections::HashMap;
use csv;
use serde::{Deserialize, Serialize};

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

#[derive(Clone, Deserialize, Serialize)]
struct Member {
    fname: String,
    lname: String,
    id: u32,
    #[serde(default)]
    items: Option<Box<LiItem>>,
}

struct Library {
    items: HashMap<u32, LiItem>,
    members: HashMap<u32, Member>,
}

impl Library {
    fn new() -> Library {
        Library {
            items: HashMap::new(),
            members: HashMap::new(),
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
    
    fn book_issue(&mut self, title_id: u32, fname: &str, lname: &str) -> Result<(), String> {
        if let Some(item) = self.items.get_mut(&title_id) {
            if item.avail_copies > 0 {
                let member_id = self.members.len() as u32 + 1;
                let member = Member {
                    fname: fname.to_string(),
                    lname: lname.to_string(),
                    id: member_id,
                    items: Some(Box::new(item.clone())),
                };

                item.avail_copies -= 1;
                self.members.insert(member_id, member);
                Ok(())
            } else {
                Err("No available copies left!".to_string())
            }
        } else {
            Err("Invalid Item ID!".to_string())
        }
    }

    fn book_return(&mut self, member_id: u32) -> Result<LiItem, String> {
        if let Some(member) = self.members.remove(&member_id) {
            if let Some(book) = member.items {
                if let Some(item) = self.items.get_mut(&book.id) {
                    item.avail_copies += 1;
                    Ok(*book)
                } else {
                    Err("Book not found in library items".to_string())
                }
            } else {
                Err("No book checked out by this member".to_string())
            }
        } else {
            Err("Member not found".to_string())
        }
    }
}

fn create_library_gui() -> Application {
    let app = Application::builder()
    .application_id("com.example.Library")
    .build();
    
    app.connect_activate(|app| {
        // Create a shared library state
        let library = Rc::new(RefCell::new(Library::new()));
        
        // Try to initialize library from CSV
        {
            let mut lib = library.borrow_mut();
            // Option 1: Add current directory path
            match lib.initialize_lib("items.csv") {
                Ok(_) => println!("Library initialized successfully"),
                Err(e) => {
                    eprintln!("Failed to initialize library: {}", e);
                    // Option 2: Print current working directory
                    match std::env::current_dir() {
                        Ok(path) => println!("Current working directory: {}", path.display()),
                        Err(_) => println!("Could not determine current directory"),
                    }
                }
            }
        }

        // Main Window
        let window = ApplicationWindow::builder()
            .application(app)
            .title("Library Management System")
            .default_width(800)
            .default_height(600)
            .build();

        // Main vertical box
        let main_box = GtkBox::new(gtk::Orientation::Vertical, 10);
        main_box.set_margin_top(10);
        main_box.set_margin_bottom(10);
        main_box.set_margin_start(10);
        main_box.set_margin_end(10);

        // Notebook for different sections
        let notebook = gtk::Notebook::new();

        // Book Issuance Page
        let issue_box = create_issue_page(Rc::clone(&library));
        notebook.append_page(&issue_box, Some(&Label::new(Some("Issue Books"))));

        // Book Return Page
        let return_box = create_return_page(Rc::clone(&library));
        notebook.append_page(&return_box, Some(&Label::new(Some("Return Books"))));

        // Member Details Page
        let member_box = create_member_details_page(Rc::clone(&library));
        notebook.append_page(&member_box, Some(&Label::new(Some("Member Details"))));

        // Library Catalog Page
        let catalog_box = create_catalog_page(Rc::clone(&library));
        notebook.append_page(&catalog_box, Some(&Label::new(Some("Library Catalog"))));

        main_box.append(&notebook);
        window.set_child(Some(&main_box));
        window.show();
    });

    app
}

fn create_issue_page(library: Rc<RefCell<Library>>) -> GtkBox {
    let issue_box = GtkBox::new(gtk::Orientation::Vertical, 10);
    
    // Item ID Entry
    let id_label = Label::new(Some("Item ID:"));
    let id_entry = Entry::new();
    
    // First Name Entry
    let fname_label = Label::new(Some("First Name:"));
    let fname_entry = Entry::new();
    
    // Last Name Entry
    let lname_label = Label::new(Some("Last Name:"));
    let lname_entry = Entry::new();
    
    // Status Label
    let status_label = Label::new(None);
    
    // Issue Button
    let issue_button = Button::with_label("Issue Book");
    issue_button.connect_clicked(glib::clone!(
        #[weak]
        id_entry,
        #[weak]
        fname_entry,
        #[weak]
        lname_entry,
        #[weak]
        status_label,
        #[weak]
        library,
        move |_| {
            let id_text = id_entry.text().to_string();
            let fname_text = fname_entry.text().to_string();
            let lname_text = lname_entry.text().to_string();
            
            if let Ok(id) = id_text.parse::<u32>() {
                let mut lib = library.borrow_mut();
                match lib.book_issue(id, &fname_text, &lname_text) {
                    Ok(_) => {
                        status_label.set_text("Book issued successfully!");
                        id_entry.set_text("");
                        fname_entry.set_text("");
                        lname_entry.set_text("");
                    },
                    Err(e) => status_label.set_text(&format!("Error: {}", e))
                }
            } else {
                status_label.set_text("Invalid Item ID");
            }
        }
    ));
    
    // Add widgets to box
    issue_box.append(&id_label);
    issue_box.append(&id_entry);
    issue_box.append(&fname_label);
    issue_box.append(&fname_entry);
    issue_box.append(&lname_label);
    issue_box.append(&lname_entry);
    issue_box.append(&issue_button);
    issue_box.append(&status_label);
    
    issue_box
}

fn create_return_page(library: Rc<RefCell<Library>>) -> GtkBox {
    let return_box = GtkBox::new(gtk::Orientation::Vertical, 10);
    
    // Member ID Entry
    let id_label = Label::new(Some("Member ID:"));
    let id_entry = Entry::new();
    
    // Status Label
    let status_label = Label::new(None);
    
    // Returned Book Details Label
    let book_details_label = Label::new(None);
    
    // Return Button
    let return_button = Button::with_label("Return Book");
    return_button.connect_clicked(glib::clone!(
        #[weak]
        id_entry,
        #[weak]
        status_label,
        #[weak]
        book_details_label,
        #[weak]
        library,
        move |_| {
            let id_text = id_entry.text().to_string();
            
            if let Ok(id) = id_text.parse::<u32>() {
                let mut lib = library.borrow_mut();
                match lib.book_return(id) {
                    Ok(book) => {
                        status_label.set_text("Book returned successfully!");
                        book_details_label.set_text(&format!(
                            "Returned Book: {} (ID: {})", 
                            book.title, 
                            book.id
                        ));
                        id_entry.set_text("");
                    },
                    Err(e) => {
                        status_label.set_text(&format!("Error: {}", e));
                        book_details_label.set_text("");
                    }
                }
            } else {
                status_label.set_text("Invalid Member ID");
            }
        }
    ));
    
    // Add widgets to box
    return_box.append(&id_label);
    return_box.append(&id_entry);
    return_box.append(&return_button);
    return_box.append(&status_label);
    return_box.append(&book_details_label);
    
    return_box
}

fn create_member_details_page(library: Rc<RefCell<Library>>) -> GtkBox {
    let member_box = GtkBox::new(gtk::Orientation::Vertical, 10);
    
    // Create a list store for members
    let list_store = ListStore::new(&[
        String::static_type(),  // First Name
        String::static_type(),  // Last Name
        u32::static_type(),     // Member ID
        String::static_type(),  // Book Title
    ]);
    
    // Create TreeView
    let tree_view = TreeView::with_model(&list_store);
    
    // Create columns
    let columns = [
        ("First Name", 0),
        ("Last Name", 1),
        ("Member ID", 2),
        ("Book Title", 3),
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
            let lib = library.borrow();
            for member in lib.members.values() {
                let book_title = member.items.as_ref()
                    .map(|b| b.title.clone())
                    .unwrap_or_else(|| "No Book".to_string());
                
                list_store.insert_with_values(None, &[
                    (0, &member.fname),
                    (1, &member.lname),
                    (2, &member.id),
                    (3, &book_title),
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
fn create_catalog_page(library: Rc<RefCell<Library>>) -> GtkBox {
    let catalog_box = GtkBox::new(gtk::Orientation::Vertical, 10);
    
    let list_store = ListStore::new(&[
        u32::static_type(),     // Item ID
        String::static_type(),  // Title
        String::static_type(),  // Author
        u32::static_type(),     // Year
        String::static_type(),  // Format
        u32::static_type(),     // Total Copies
        u32::static_type(),     // Available Copies
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
            ]);
        }
    };

    // Populate catalog on startup
    {
        let lib = library.borrow();
        refresh_catalog(&list_store, &lib);
    }

    refresh_button.connect_clicked(glib::clone!(
        #[weak]
        list_store,
        #[weak]
        library,
        move |_| {
            let lib = library.borrow();
            refresh_catalog(&list_store, &lib);
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