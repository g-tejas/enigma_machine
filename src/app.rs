use crate::{plot::candlestick_chart, utils, widgets::Widget};
use barter_data::model::MarketEvent;
use eframe::egui;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc::{Receiver, Sender};

#[allow(dead_code)]
pub struct State<'a> {
    // HashSet, so only unique widget names are stored.
    // TODO: Add unique identifier to widget names OrderBook::spot::BTCUSD for example.
    // This is so we can have multiple orderbooks up at the same time
    // Style-related fields
    open_tabs: HashSet<String>,
    style: Option<egui_dock::Style>,
    lock_layout: bool,

    // Lock-free!
    tx: Sender<MarketEvent>,
    rx: Receiver<MarketEvent>,
    events: VecDeque<MarketEvent>,
    gizmos: HashMap<&'a str, Box<dyn Widget>>, // Vector of pointers to a trait value Widget, might change to Hashmap
}

impl egui_dock::TabViewer for State<'_> {
    type Tab = String;

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab.as_str() {
            "Welcome" => self.candlestick_chart(ui),
            "Portfolio" => {
                self.gizmos
                    // it has to be get_mut() not get() because get() returns a & ref not a &mut
                    .get_mut("Chart")
                    .unwrap()
                    .show(ui, &mut self.events, self.tx.clone())
            }
            "Machine Configuration" => self.machine_config(ui),
            "Orderbook" => self.gizmos.get_mut("💸 Aggregated Trades").unwrap().show(
                ui,
                &mut self.events,
                self.tx.clone(),
            ),
            _ => {
                ui.label(tab.as_str());
            }
        }
    }

    // when you right click a tab
    fn context_menu(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab.as_str() {
            "Orderbook" => ui.label("We gon add some fancy shit here"),
            _ => ui.label("helo"),
        };
    }

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.as_str().into()
    }

    fn on_close(&mut self, tab: &mut Self::Tab) -> bool {
        self.open_tabs.remove(tab);
        true
    }
}

impl State<'_> {
    // The only things that should be stored here are styling / open_tabs related stuff
    // since the things that can be accessed from self, are very limited. Or we can store the financial
    // data here itself
    fn candlestick_chart(&mut self, ui: &mut egui::Ui) {
        candlestick_chart(ui);
        // egui::Window::new("Hello").show(ui.ctx(), |ui| {
        //     ui.label("Hgello world");
        // });
    }

    fn machine_config(&mut self, ui: &mut egui::Ui) {
        ui.heading("Machine Configuration");
        // let style = self.style.as_mut().unwrap();

        ui.collapsing("Aesthetics", |ui| {
            ui.separator();
            ui.label("Edit shit here");
            // ui.checkbox(&mut style.tabs_are_draggable, "Tabs are draggable");
        });
    }
}

pub struct Machine<'a> {
    state: State<'a>,
    tree: egui_dock::Tree<String>,
}

impl Machine<'_> {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        #[allow(unused_mut)]
        let mut default = Self::default();

        #[cfg(feature = "persistence")]
        if let Some(storage) = cc.storage {
            if let Some(state) = eframe::get_value(storage, eframe::APP_KEY) {
                default.state = state;
            }
        }

        utils::configure_fonts(&cc.egui_ctx);
        default
    }
}

impl Default for Machine<'_> {
    // Default Layout
    fn default() -> Self {
        let mut tree = egui_dock::Tree::new(vec![
            "Welcome".to_owned(),
            "Machine Configuration".to_owned(),
        ]);
        let [a, _b] = tree.split_left(
            egui_dock::NodeIndex::root(),
            0.4,
            vec!["Orderbook".to_owned()],
        );
        let [_, _] = tree.split_below(a, 0.5, vec!["Portfolio".to_owned()]);
        let mut open_tabs = HashSet::new();
        for node in tree.iter() {
            if let egui_dock::Node::Leaf { tabs, .. } = node {
                for tab in tabs {
                    open_tabs.insert(tab.clone());
                }
            }
        }

        // Create channel for different components to communicate
        let (tx, rx) = std::sync::mpsc::channel();

        // Create a Hashmap of widgets
        let aggr_trades_widget: Box<dyn Widget> =
            Box::new(crate::widgets::aggr_trades::AggrTrades::default());
        let chart_widget: Box<dyn Widget> = Box::new(crate::widgets::chart::Chart::default());
        let mut gizmos: HashMap<&str, Box<dyn Widget>> = HashMap::new();
        gizmos.insert(aggr_trades_widget.name(), aggr_trades_widget);
        gizmos.insert(chart_widget.name(), chart_widget);

        let state = State {
            open_tabs,
            style: None,
            lock_layout: false,
            tx,
            rx,
            events: VecDeque::new(),
            gizmos,
        };
        Self { state, tree }
    }
}

impl eframe::App for Machine<'_> {
    #[cfg(feature = "persistence")]
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.state);
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Here's where we receive data from transmitter
        if let Ok(event) = self.state.rx.try_recv() {
            self.state.events.push_front(event);
            self.state.events.truncate(50);
        }

        // Menu bar
        egui::TopBottomPanel::top("egui_dock::MenuBar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                egui::widgets::global_dark_light_mode_switch(ui);
                ui.separator();
                ui.menu_button("⚙", |ui| {
                    if ui.button("Quit").clicked() {
                        frame.close();
                    }
                });

                ui.menu_button("Widgets", |ui| {
                    // allow certain tabs to be toggled
                    for tab in &["Welcome", "Portfolio"] {
                        if ui
                            .selectable_label(self.state.open_tabs.contains(*tab), *tab)
                            .clicked()
                        {
                            if let Some(index) = self.tree.find_tab(&tab.to_string()) {
                                self.tree.remove_tab(index);
                                self.state.open_tabs.remove(*tab);
                            } else {
                                self.tree.push_to_focused_leaf(tab.to_string());
                            }

                            ui.close_menu();
                        }
                    }
                    // Not using checkbox since we want to be able to add more than one tabs
                    // for tab in &["Welcome", "Portfolio", "Watchlist", "Depth Chart"] {
                    //     ui.checkbox(&mut self.context.open_tabs.contains(*tab), *tab);
                    //     // ui.selectable_label(self.context.open_tabs.contains(*tab), *tab);
                    // }
                    ui.label("This is where we will add the various tabs");
                });
            })
        });

        let panel_config = egui::containers::Frame {
            inner_margin: egui::style::Margin {
                left: 4.,
                right: 7.,
                top: 0.,
                bottom: 3.,
            },
            fill: egui::Color32::from_rgba_unmultiplied(25, 25, 25, 200),
            ..Default::default()
        };

        // Add the "workspaces feature here" > more deets in the README.md
        egui::TopBottomPanel::bottom("bottom_panel")
            .exact_height(25.)
            .resizable(false)
            .frame(panel_config)
            .show(ctx, |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.selectable_label(self.state.lock_layout, "🔒").clicked() {
                        // let style = self.state.style.as_mut().unwrap();
                        // ui.checkbox(&mut style.tabs_are_draggable, "Lock");
                        self.state.lock_layout = !self.state.lock_layout;
                        println!("locked");
                    }
                });
            });

        // This is where the tabs and widgets are shown
        egui::CentralPanel::default().show(ctx, |_ui| {
            egui_dock::DockArea::new(&mut self.tree)
                .style(
                    egui_dock::StyleBuilder::from_egui(ctx.style().as_ref())
                        .show_add_buttons(true)
                        .show_add_popup(true)
                        .with_separator_color_hovered(egui::Color32::LIGHT_BLUE)
                        .with_border_color(egui::Color32::RED)
                        .expand_tabs(true)
                        .build(),
                )
                .show(ctx, &mut self.state);
        });

        // Call repaint every update
        ctx.request_repaint();
    }
}
