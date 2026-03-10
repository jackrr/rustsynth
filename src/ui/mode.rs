#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UIMode {
    Voices,
    FxGroups,
    Routing,
}

impl UIMode {
    pub fn tab_label(&self) -> &str {
        match self {
            UIMode::Voices => "1:VOICES",
            UIMode::FxGroups => "2:FX GROUPS",
            UIMode::Routing => "3:ROUTING",
        }
    }
}
