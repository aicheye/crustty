use ratatui::style::Color;

pub struct Theme {
    #[allow(dead_code)] // Background color field for future use
    pub bg: Color,
    pub fg: Color,
    pub primary: Color,   // Blue
    pub secondary: Color, // Orange
    pub comment: Color,   // Grey
    pub success: Color,   // Green
    pub error: Color,     // Red
    pub keyword: Color,
    pub string: Color,
    pub number: Color,
    pub border_focused: Color,
    pub border_normal: Color,
    pub current_line_bg: Color,
    pub function: Color,
    pub muted_function: Color, // Muted yellow for call chain functions
    pub type_name: Color,      // Cyan for type names
    pub return_value: Color,   // Special color for return values
}

pub const DEFAULT_THEME: Theme = Theme {
    bg: Color::Rgb(30, 30, 46),
    fg: Color::Rgb(205, 214, 244),
    primary: Color::Rgb(137, 180, 250),   // Blue
    secondary: Color::Rgb(250, 179, 135), // Orange
    comment: Color::Rgb(108, 112, 134),
    success: Color::Rgb(166, 227, 161),
    error: Color::Rgb(243, 139, 168),
    keyword: Color::Rgb(137, 180, 250),        // Blue for keywords
    string: Color::Rgb(250, 179, 135),         // Orange for strings
    number: Color::Rgb(250, 179, 135),         // Orange for numbers
    border_focused: Color::Rgb(249, 226, 175), // Yellow border for focus
    border_normal: Color::Rgb(108, 112, 134),  // Grey border for normal
    current_line_bg: Color::Rgb(50, 50, 70),   // Slightly lighter BG for current line
    function: Color::Rgb(249, 226, 175),       // Yellow for functions
    muted_function: Color::Rgb(180, 165, 120), // Muted yellow for call chain
    type_name: Color::Rgb(148, 226, 213),      // Cyan/teal for type names
    return_value: Color::Rgb(245, 194, 231),   // Pink for return values
};
