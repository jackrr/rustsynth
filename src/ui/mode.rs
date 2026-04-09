#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UIMode {
    Voices,
    FxGroups,
    Sequencer,
}

impl UIMode {
    pub fn tab_label(&self) -> &str {
        match self {
            UIMode::Voices => "1:VOICES",
            UIMode::FxGroups => "2:FX GROUPS",
            UIMode::Sequencer => "3:SEQUENCER",
        }
    }
}
