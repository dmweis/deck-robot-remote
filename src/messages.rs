use chrono::prelude::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct InputMessage {
    pub gamepads: HashMap<usize, GamepadMessage>,
    pub time: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Serialize, Default, JsonSchema)]
pub struct GamepadMessage {
    pub name: String,
    pub connected: bool,
    pub last_event_time: DateTime<Utc>,
    pub button_down_event_counter: BTreeMap<Button, usize>,
    pub button_up_event_counter: BTreeMap<Button, usize>,
    pub button_down: BTreeMap<Button, bool>,
    pub axis_state: BTreeMap<Axis, f32>,
}

#[derive(
    Debug, Deserialize, Serialize, PartialEq, Eq, Hash, PartialOrd, Ord, Clone, Copy, JsonSchema,
)]
pub enum Button {
    South,
    East,
    North,
    West,
    C,
    Z,
    LeftTrigger,
    LeftTrigger2,
    RightTrigger,
    RightTrigger2,
    Select,
    Start,
    Mode,
    LeftThumb,
    RightThumb,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
    Unknown,
    LeftPaddle,
    RightPaddle,
}

impl Button {
    pub fn all_gilrs_buttons() -> &'static [gilrs::ev::Button] {
        &[
            gilrs::ev::Button::South,
            gilrs::ev::Button::East,
            gilrs::ev::Button::North,
            gilrs::ev::Button::West,
            gilrs::ev::Button::C,
            gilrs::ev::Button::Z,
            gilrs::ev::Button::LeftTrigger,
            gilrs::ev::Button::LeftTrigger2,
            gilrs::ev::Button::RightTrigger,
            gilrs::ev::Button::RightTrigger2,
            gilrs::ev::Button::Select,
            gilrs::ev::Button::Start,
            gilrs::ev::Button::Mode,
            gilrs::ev::Button::LeftThumb,
            gilrs::ev::Button::RightThumb,
            gilrs::ev::Button::DPadUp,
            gilrs::ev::Button::DPadDown,
            gilrs::ev::Button::DPadLeft,
            gilrs::ev::Button::DPadRight,
        ]
    }
}

impl From<gilrs::ev::Button> for Button {
    fn from(value: gilrs::ev::Button) -> Self {
        match value {
            gilrs::ev::Button::South => Button::South,
            gilrs::ev::Button::East => Button::East,
            gilrs::ev::Button::North => Button::North,
            gilrs::ev::Button::West => Button::West,
            gilrs::ev::Button::C => Button::C,
            gilrs::ev::Button::Z => Button::Z,
            gilrs::ev::Button::LeftTrigger => Button::LeftTrigger,
            gilrs::ev::Button::LeftTrigger2 => Button::LeftTrigger2,
            gilrs::ev::Button::RightTrigger => Button::RightTrigger,
            gilrs::ev::Button::RightTrigger2 => Button::RightTrigger2,
            gilrs::ev::Button::Select => Button::Select,
            gilrs::ev::Button::Start => Button::Start,
            gilrs::ev::Button::Mode => Button::Mode,
            gilrs::ev::Button::LeftThumb => Button::LeftThumb,
            gilrs::ev::Button::RightThumb => Button::RightThumb,
            gilrs::ev::Button::DPadUp => Button::DPadUp,
            gilrs::ev::Button::DPadDown => Button::DPadDown,
            gilrs::ev::Button::DPadLeft => Button::DPadLeft,
            gilrs::ev::Button::DPadRight => Button::DPadRight,
            gilrs::ev::Button::Unknown => Button::Unknown,
        }
    }
}

#[derive(
    Debug, Deserialize, Serialize, PartialEq, Eq, Hash, PartialOrd, Ord, Clone, Copy, JsonSchema,
)]
pub enum Axis {
    LeftStickX,
    LeftStickY,
    LeftZ,
    RightStickX,
    RightStickY,
    RightZ,
    DPadX,
    DPadY,
    Unknown,
}

impl Axis {
    #[allow(unused)]
    pub fn all_axes() -> &'static [Axis] {
        &[
            Axis::LeftStickX,
            Axis::LeftStickY,
            Axis::LeftZ,
            Axis::RightStickX,
            Axis::RightStickY,
            Axis::RightZ,
            Axis::DPadX,
            Axis::DPadY,
        ]
    }
}

impl From<gilrs::ev::Axis> for Axis {
    fn from(value: gilrs::ev::Axis) -> Self {
        match value {
            gilrs::ev::Axis::LeftStickX => Axis::LeftStickX,
            gilrs::ev::Axis::LeftStickY => Axis::LeftStickY,
            gilrs::ev::Axis::LeftZ => Axis::LeftZ,
            gilrs::ev::Axis::RightStickX => Axis::RightStickX,
            gilrs::ev::Axis::RightStickY => Axis::RightStickY,
            gilrs::ev::Axis::RightZ => Axis::RightZ,
            gilrs::ev::Axis::DPadX => Axis::DPadX,
            gilrs::ev::Axis::DPadY => Axis::DPadY,
            gilrs::ev::Axis::Unknown => Axis::Unknown,
        }
    }
}
