#[derive(Debug)]
pub struct TuiApp {
    pub client_scroll: usize,
    pub download_scroll: usize,
    pub active_panel: Panel,
}

#[derive(Debug, PartialEq)]
pub enum Panel {
    Clients,
    Downloads,
}

impl Default for TuiApp {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiApp {
    pub fn new() -> Self {
        Self {
            client_scroll: 0,
            download_scroll: 0,
            active_panel: Panel::Clients,
        }
    }

    pub fn scroll_up(&mut self) {
        match self.active_panel {
            Panel::Clients => self.client_scroll = self.client_scroll.saturating_sub(1),
            Panel::Downloads => self.download_scroll = self.download_scroll.saturating_sub(1),
        }
    }

    pub fn scroll_down(&mut self) {
        match self.active_panel {
            Panel::Clients => self.client_scroll += 1,
            Panel::Downloads => self.download_scroll += 1,
        }
    }

    pub fn next_panel(&mut self) {
        self.active_panel = match self.active_panel {
            Panel::Clients => Panel::Downloads,
            Panel::Downloads => Panel::Clients,
        };
    }
}
