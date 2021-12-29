use {
    crate::{
        error::Result,
        get_obj,
        source::{self, Feed},
    },
    gtk::{
        glib::{clone, MainContext, PRIORITY_DEFAULT},
        prelude::*,
        AboutDialog, Align, ApplicationWindow, Box as GtkBox, Builder, Button, Dialog, Entry,
        EventBox, IconSize, Image, InfoBar, Label, ListBox, MessageType, Orientation, ResponseType,
        Stack,
    },
    std::{rc::Rc, thread},
};

#[derive(Debug)]
enum UiEvent {
    AddSource(String),
    Reload,
    ReloadComplete(Vec<Feed>),
    Message(String, MessageType),
}

#[derive(Debug)]
pub struct Ui {
    main_window: ApplicationWindow,
    stack: Stack,
    about_dialog: AboutDialog,
    show_about_dialog_btn: Button,
    add_source_btn: Button,
    add_source_entry: Entry,
    add_source_dialog: Dialog,
    refresh_btn: Button,
    sources_list: ListBox,
    posts_list: ListBox,
    info_bar: InfoBar,
    info_bar_label: Label,
}

impl Ui {
    pub fn new() -> Rc<Self> {
        let b = Builder::from_resource(resource!("ui/main"));
        let inner = Rc::new(Self {
            main_window: get_obj!(b, "main-window"),
            stack: get_obj!(b, "stack"),
            about_dialog: get_obj!(b, "about-dialog"),
            show_about_dialog_btn: get_obj!(b, "show-about-dialog-btn"),
            add_source_btn: get_obj!(b, "add-source-btn"),
            add_source_entry: get_obj!(b, "add-source-entry"),
            add_source_dialog: get_obj!(b, "add-source-dialog"),
            refresh_btn: get_obj!(b, "refresh-btn"),
            sources_list: get_obj!(b, "sources-list"),
            posts_list: get_obj!(b, "posts-list"),
            info_bar: get_obj!(b, "info-bar"),
            info_bar_label: get_obj!(b, "info-bar-label"),
        });

        let (tx, rx) = MainContext::channel::<UiEvent>(PRIORITY_DEFAULT);

        rx.attach(
            None,
            clone!(@strong inner, @strong tx => move |evt| {
                match evt {
                    UiEvent::AddSource(source) => {
                        match source::add_source(&source) {
                            Ok(_) => {
                                let _ = tx.send(UiEvent::Message(format!("Added source {}", source), MessageType::Info));
                                let _ = tx.send(UiEvent::Reload);
                            },
                            Err(e) => {
                                let _ = tx.send(UiEvent::Message(format!("{:?}", e), MessageType::Error));
                            }
                        }
                    },
                    UiEvent::Message(msg, kind) => {
                        inner.info_bar_label.set_text(msg.as_str());
                        inner.info_bar.set_message_type(kind);
                        inner.info_bar.set_revealed(true);
                    },
                    UiEvent::Reload => {
                        inner.stack.set_visible_child_name("loading");
                        thread::spawn(clone!(@strong tx => move || {
                            let result: Result<Vec<Feed>> = (|| {
                                let sources = source::get_sources()?;
                                let (feeds, errors) = source::download(sources.iter().map(|a| a.as_str()).collect::<Vec<_>>().as_slice());
                                for e in errors {
                                    let _ = tx.send(UiEvent::Message(format!("{:?}", e), MessageType::Error));
                                }
                                Ok(feeds)
                            })();
                            match result {
                                Ok(feeds) => {
                                    let _ = tx.send(UiEvent::ReloadComplete(feeds));
                                }
                                Err(e) => {
                                    let _ = tx.send(UiEvent::Message(format!("{:?}", e), MessageType::Error));
                                }
                            }
                        }));
                    }
                    UiEvent::ReloadComplete(feeds) => {
                        for w in &inner.sources_list.children() {
                            inner.sources_list.remove(w);
                        }
                        for w in &inner.posts_list.children() {
                            inner.posts_list.remove(w);
                        }

                        for feed in &feeds {
                            let row = GtkBox::new(Orientation::Horizontal, 6);
                            row.add(&Image::from_icon_name(Some("emblem-documents"), IconSize::LargeToolbar));
                            let label = Label::new(Some(&feed.title));
                            row.add(&label);
                            row.set_border_width(6);
                            row.show_all();
                            inner.sources_list.add(&row);
                        }

                        let mut posts = feeds.into_iter().fold(Vec::new(), |mut acc, feed| {
                            for p in feed.posts.into_iter() {
                                acc.push(p);
                            }
                            acc
                        });

                        posts.sort_by(|a, b| b.published.cmp(&a.published));

                        for post in posts {
                            let row = EventBox::new();
                            row.connect_button_press_event(clone!(@strong post.url as url => move |_, _| {
                                let _ = gtk::show_uri_on_window(None::<&ApplicationWindow>, &url, 0);
                                Inhibit(false)
                            }));
                            let box1 = GtkBox::new(Orientation::Vertical, 6);
                            box1.set_border_width(6);
                            let label1 = Label::new(Some(&post.title));
                            label1.set_halign(Align::Start);
                            let label2 = Label::new(Some(&format!("{} - {}", post.published, post.publisher)));
                            label2.set_halign(Align::Start);
                            box1.add(&label1);
                            box1.add(&label2);
                            row.add(&box1);
                            row.show_all();
                            inner.posts_list.add(&row);
                        }

                        inner.stack.set_visible_child_name("list");
                    }
                }
                Continue(true)
            }),
        );

        inner
            .add_source_btn
            .connect_clicked(clone!(@strong inner, @strong tx => move |_| {
                match inner.add_source_dialog.run() {
                    ResponseType::Ok => {
                        let input = inner.add_source_entry.text().to_string();
                        if !input.trim().is_empty() {
                            let _ = tx.send(UiEvent::AddSource(input));
                        }
                        inner.add_source_dialog.hide()
                    },
                    _ => inner.add_source_dialog.hide()
                }
            }));

        inner
            .add_source_dialog
            .add_buttons(&[("Ok", ResponseType::Ok), ("Cancel", ResponseType::Cancel)]);

        inner
            .refresh_btn
            .connect_clicked(clone!(@strong tx => move |_| {
                let _ = tx.send(UiEvent::Reload);
            }));

        inner
            .show_about_dialog_btn
            .connect_clicked(clone!(@strong inner => move |_| {
                inner.about_dialog.run();
                inner.about_dialog.hide();
            }));

        inner
            .info_bar
            .connect_response(|ib, _| ib.set_revealed(false));

        let _ = tx.send(UiEvent::Reload);

        inner
    }

    pub fn set_app(&self, app: &gtk::Application) {
        self.main_window.set_application(Some(app));
    }

    pub fn show(&self) {
        self.main_window.show_all();
    }
}
