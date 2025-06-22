use lazy_static::lazy_static;

lazy_static! {
    pub static ref ERROR_COLOR: iced::Color = iced::Color::from_rgb(0.8, 0.1, 0.1);
    pub static ref ADDED_COLOR: iced::Color = iced::Color::from_rgb(0.1, 0.8, 0.1);
    pub static ref REMOVED_COLOR: iced::Color = iced::Color::from_rgb(0.8, 0.1, 0.8);
    pub static ref MODIFIED_COLOR: iced::Color = iced::Color::from_rgb(0.1, 0.1, 0.8);
}
