// SPDX-License-Identifier: {{LICENSE}}

use crate::config::Config;
use crate::fl;
use crate::package::{install_packages_local, Package};
use crate::packagekit::{transaction_handle, PackageKit};
use ashpd::desktop::file_chooser::{FileFilter, SelectedFiles};
use cosmic::app::{Command, Core};
use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::iced::alignment::{Horizontal, Vertical};
use cosmic::iced::{Alignment, Length, Subscription};
use cosmic::prelude::CollectionWidget;
use cosmic::widget::{self, menu, settings};
use cosmic::{command, cosmic_theme, theme, Application, ApplicationExt, Element};
use futures_util::SinkExt;
use std::collections::HashMap;

const REPOSITORY: &str = "https://github.com/cosmic-utils/wizard";
const APP_ICON: &[u8] = include_bytes!("../res/icons/hicolor/scalable/apps/icon.svg");

/// The application model stores app-specific state used to describe its interface and
/// drive its logic.
pub struct AppModel {
    /// Application state which is managed by the COSMIC runtime.
    core: Core,
    /// Display a context drawer with the designated page if defined.
    context_page: ContextPage,
    /// Key bindings for the application's menu bar.
    key_binds: HashMap<menu::KeyBind, MenuAction>,
    // Configuration data that persists between application runs.
    config: Config,

    package: Option<Package>,
    is_installed: bool,
}

/// Messages emitted by the application and its widgets.
#[derive(Debug, Clone)]
pub enum Message {
    OpenRepositoryUrl,
    SubscriptionChannel,
    ToggleContextPage(ContextPage),
    UpdateConfig(Config),
    SelectFile,
    UpdatePackage(String),
    AskInstallation(Box<Package>),
    PackageInstalled(bool),
}

/// Create a COSMIC application from the app model
impl Application for AppModel {
    /// The async executor that will be used to run your application's commands.
    type Executor = cosmic::executor::Default;

    /// Data that your application receives to its init method.
    type Flags = ();

    /// Messages which the application and its widgets will emit.
    type Message = Message;

    /// Unique identifier in RDNN (reverse domain name notation) format.
    const APP_ID: &'static str = "io.github.cosmicUtils.Wizard";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    /// Initializes the application with any given flags and startup commands.
    fn init(core: Core, _flags: Self::Flags) -> (Self, Command<Self::Message>) {
        // Construct the app model with the runtime's core.
        let mut app = AppModel {
            core,
            context_page: ContextPage::default(),
            key_binds: HashMap::new(),
            // Optional configuration file for an application.
            config: cosmic_config::Config::new(Self::APP_ID, Config::VERSION)
                .map(|context| match Config::get_entry(&context) {
                    Ok(config) => config,
                    Err((_errors, config)) => {
                        // for why in errors {
                        //     tracing::error!(%why, "error loading app config");
                        // }

                        config
                    }
                })
                .unwrap_or_default(),

            package: None,
            is_installed: false,
        };

        // Create a startup command that sets the window title.
        let command = app.update_title();

        (app, command)
    }

    /// Elements to pack at the start of the header bar.
    fn header_start(&self) -> Vec<Element<Self::Message>> {
        let menu_bar = menu::bar(vec![menu::Tree::with_children(
            menu::root(fl!("view")),
            menu::items(
                &self.key_binds,
                vec![menu::Item::Button(fl!("about"), MenuAction::About)],
            ),
        )]);

        vec![menu_bar.into()]
    }

    /// Display a context drawer if the context page is requested.
    fn context_drawer(&self) -> Option<Element<Self::Message>> {
        if !self.core.window.show_context {
            return None;
        }

        Some(match self.context_page {
            ContextPage::About => self.about(),
        })
    }

    /// Describes the interface based on the current state of the application model.
    ///
    /// Application events will be processed through the view. Any messages emitted by
    /// events received by widgets will be passed to the update method.
    fn view(&self) -> Element<Self::Message> {
        let filechooser_btn =
            widget::button::standard(fl!("select-file")).on_press(Message::SelectFile);

        let install_btn: Option<Element<'_, _>> = self.package.clone().map(|package| {
            let mut btn = widget::button::suggested(fl!("install-file"));

            if !self.is_installed {
                btn = btn.on_press(Message::AskInstallation(Box::new(package)));
            }

            btn.into()
        });

        let header = widget::container(
            widget::row()
                .spacing(60)
                .push(filechooser_btn)
                .push_maybe(install_btn),
        )
        .width(Length::Fill)
        .align_x(Horizontal::Center);

        let details: Option<Element<'_, _>> = self.package.clone().map(|package| {
            let column = widget::list_column()
                .add(settings::item("ID", widget::text(package.id)))
                .add(settings::item("Name", widget::text(package.name)))
                .add(settings::item("Version", widget::text(package.version)))
                .add(settings::item(
                    "Architecture",
                    widget::text(package.architecture),
                ))
                .add(settings::item("Summary", widget::text(package.summary)))
                .add(settings::item(
                    "Description",
                    widget::text(package.description),
                ))
                .add(settings::item("URL", widget::text(package.url)))
                .add(settings::item("License", widget::text(package.license)))
                .add(settings::item("Size", widget::text(package.size)));

            widget::container(widget::container(column).max_width(800))
                .align_x(Horizontal::Center)
                .into()
        });

        let content = widget::column()
            .spacing(16)
            .push(header)
            .push_maybe(details);

        widget::container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center)
            .into()
    }

    /// Register subscriptions for this application.
    ///
    /// Subscriptions are long-running async tasks running in the background which
    /// emit messages to the application through a channel. They are started at the
    /// beginning of the application, and persist through its lifetime.
    fn subscription(&self) -> Subscription<Self::Message> {
        struct MySubscription;

        Subscription::batch(vec![
            // Create a subscription which emits updates through a channel.
            cosmic::iced::subscription::channel(
                std::any::TypeId::of::<MySubscription>(),
                4,
                move |mut channel| async move {
                    _ = channel.send(Message::SubscriptionChannel).await;

                    futures_util::future::pending().await
                },
            ),
            // Watch for application configuration changes.
            self.core()
                .watch_config::<Config>(Self::APP_ID)
                .map(|update| {
                    // for why in update.errors {
                    //     tracing::error!(?why, "app config error");
                    // }

                    Message::UpdateConfig(update.config)
                }),
        ])
    }

    /// Handles messages emitted by the application and its widgets.
    ///
    /// Commands may be returned for asynchronous execution of code in the background
    /// on the application's async runtime.
    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::OpenRepositoryUrl => {
                _ = open::that_detached(REPOSITORY);
            }

            Message::SubscriptionChannel => {
                // For example purposes only.
            }

            Message::ToggleContextPage(context_page) => {
                if self.context_page == context_page {
                    // Close the context drawer if the toggled context page is the same.
                    self.core.window.show_context = !self.core.window.show_context;
                } else {
                    // Open the context drawer to display the requested context page.
                    self.context_page = context_page;
                    self.core.window.show_context = true;
                }

                // Set the title of the context drawer.
                self.set_context_title(context_page.title());
            }

            Message::UpdateConfig(config) => {
                self.config = config;
            }

            Message::SelectFile => {
                let future = async {
                    if let Ok(request) = SelectedFiles::open_file()
                        .title("Select Package to install")
                        .accept_label("Read")
                        .modal(true)
                        .filter(
                            FileFilter::new("*.deb")
                                .mimetype("application/vnd.debian.binary-package"),
                        )
                        .send()
                        .await
                    {
                        if let Ok(file) = request.response() {
                            return match file.uris().first() {
                                Some(url) => {
                                    return Some(url.path().to_string());
                                }
                                None => None,
                            };
                        }
                    }

                    None
                };

                return Command::perform(future, |path| {
                    if let Some(path) = path {
                        return cosmic::app::Message::App(Message::UpdatePackage(path));
                    }
                    cosmic::app::Message::None
                });
            }

            Message::UpdatePackage(path) => {
                let pk = PackageKit::new();
                let tx = pk.transaction().unwrap();

                tx.get_details_local(&[&path]).unwrap();

                let (tx_details, _tx_packages) = transaction_handle(tx, |_, _| {}).unwrap();

                for tx_detail in tx_details {
                    self.package = Some(Package::new(path.clone(), tx_detail));
                }
            }

            Message::AskInstallation(package) => {
                if install_packages_local(*package).is_ok() {
                    return command::future(async move { Message::PackageInstalled(true) });
                }
            }

            Message::PackageInstalled(status) => {
                self.is_installed = status;
            }
        }

        Command::none()
    }
}

impl AppModel {
    /// The about page for this app.
    pub fn about(&self) -> Element<Message> {
        let cosmic_theme::Spacing { space_xxs, .. } = theme::active().cosmic().spacing;

        let icon = widget::svg(widget::svg::Handle::from_memory(APP_ICON));

        let title = widget::text::title3(fl!("app-title"));

        let link = widget::button::link(REPOSITORY)
            .on_press(Message::OpenRepositoryUrl)
            .padding(0);

        widget::column()
            .push(icon)
            .push(title)
            .push(link)
            .align_items(Alignment::Center)
            .spacing(space_xxs)
            .into()
    }

    /// Updates the header and window titles.
    pub fn update_title(&mut self) -> Command<Message> {
        let window_title = fl!("app-title");
        self.set_window_title(window_title)
    }
}

/// The context page to display in the context drawer.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum ContextPage {
    #[default]
    About,
}

impl ContextPage {
    fn title(&self) -> String {
        match self {
            Self::About => fl!("about"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuAction {
    About,
}

impl menu::action::MenuAction for MenuAction {
    type Message = Message;

    fn message(&self) -> Self::Message {
        match self {
            MenuAction::About => Message::ToggleContextPage(ContextPage::About),
        }
    }
}
