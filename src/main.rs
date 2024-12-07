use std::io;

use crossterm::event::{self, Event, KeyCode};
use meval::Expr;
use ratatui::{
    layout::{Constraint, Direction, Flex, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Axis, Block, Borders, Chart, Clear, Dataset, Paragraph},
    DefaultTerminal, Frame,
};

const SETTINGS_LAYOUT: [[Settings; 4]; 2] = [
    [
        Settings::Function,
        Settings::LowerBound,
        Settings::UpperBound,
        Settings::RecalculateArea,
    ],
    [
        Settings::MinimumX,
        Settings::MaximumX,
        Settings::MinimumY,
        Settings::MaximumY,
    ],
];

struct App<'a> {
    function_text: String,
    expression: Expr,
    data: Vec<(f64, f64)>,
    limits_indexs: (Option<usize>, Option<usize>),
    dx: f64,
    // <Lower, Upper>
    bounds_text: Vec<String>,
    bounds: [f64; 2],
    upper_bound_line: Vec<(f64, f64)>,
    lower_bound_line: Vec<(f64, f64)>,
    x_axis_line: Vec<(f64, f64)>,
    // [Min, Max]
    window_x: [f64; 2],
    window_x_text: Vec<String>,
    window_y: [f64; 2],
    window_y_text: Vec<String>,
    area: f64,
    active_screen: CurrentScreen,
    settings_focus: &'a Settings,
    settings_position_x: usize,
    settings_position_y: usize,
    exit: bool,
}

#[derive(PartialEq, Eq)]
enum CurrentScreen {
    Main,
    Settings,
}

enum Settings {
    Function,
    LowerBound,
    UpperBound,
    RecalculateArea,
    MinimumX,
    MinimumY,
    MaximumX,
    MaximumY,
}

impl App<'_> {
    fn new() -> Self {
        Self {
            function_text: "x".to_string(),
            expression: "x".parse().unwrap(),
            data: Vec::new(),
            limits_indexs: (None, None),
            dx: 0.001,
            bounds_text: vec!["0".to_string(); 2],
            bounds: [0.0; 2],
            upper_bound_line: Vec::new(),
            lower_bound_line: Vec::new(),
            x_axis_line: Vec::new(),
            window_x: [-5.0, 5.0],
            window_x_text: vec!["-5".to_string(), "5".to_string()],
            window_y: [-10.0, 10.0],
            window_y_text: vec!["-10".to_string(), "10".to_string()],
            area: 0.0,
            active_screen: CurrentScreen::Main,
            settings_focus: &Settings::Function,
            settings_position_x: 0,
            settings_position_y: 0,
            exit: false,
        }
    }

    fn populate_data(&mut self) {
        self.data.clear();
        self.limits_indexs = (None, None);
        let function = self.expression.clone().bind("x").unwrap();
        let mut x = self.window_x[0];
        let mut i = 0;

        while x < self.window_x[1] {
            let y = function(x);

            if self.limits_indexs.0.is_none() && x >= self.bounds[0] {
                self.limits_indexs.0 = Some(i);
            } else if self.limits_indexs.1.is_none() && x >= self.bounds[1] {
                self.limits_indexs.1 = Some(i);
            }

            self.data.push((x, y));
            x += self.dx;
            i += 1;
        }

        self.populate_upper_bound_line();
        self.populate_lower_bound_line();
        self.populate_x_axis_line();

        self.calculate_area();
    }

    fn populate_upper_bound_line(&mut self) {
        self.upper_bound_line.clear();

        // Set x to the upper bound
        let x = self.bounds[1];
        let function = self.expression.clone().bind("x").unwrap();
        let height = function(x);
        let mut y = self.window_y[0];
        let step = (y - height).abs() / 100.0;

        while y < height {
            self.upper_bound_line.push((x, y));
            y += step;
        }
    }

    fn populate_lower_bound_line(&mut self) {
        self.lower_bound_line.clear();

        // Set x to lower bound
        let x = self.bounds[0];
        let function = self.expression.clone().bind("x").unwrap();
        let height = function(x);
        let mut y = self.window_y[0];
        let step = (y - height).abs() / 100.0;

        while y < height {
            self.lower_bound_line.push((x, y));
            y += step;
        }
    }

    fn populate_x_axis_line(&mut self) {
        self.x_axis_line.clear();

        let mut x = self.window_x[0];
        let step = (self.window_x[1] - self.window_x[0]).abs() / 100.0;

        while x < self.window_x[1] {
            self.x_axis_line.push((x, 0.0));
            x += step;
        }
    }

    fn calculate_area(&mut self) {
        self.area = 0.0;

        self.area = self.data[self.limits_indexs.0.unwrap()..self.limits_indexs.1.unwrap()]
            .windows(2)
            .map(|window| {
                let ((_, y1), (_, y2)) = (window[0], window[1]);
                self.dx * (y2 + y1) / 2.0
            })
            .sum::<f64>();
    }

    fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame)).unwrap();
            self.handle_events().unwrap();
        }

        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(3),
            ])
            .split(frame.area());

        let title_block = Block::default()
            .borders(Borders::ALL)
            .style(Style::default());

        let title = Paragraph::new(Text::styled(
            "Welcome to Mimmob07's numerical integration calculator",
            Style::default().fg(Color::Green),
        ))
        .block(title_block);

        let area_footer = Paragraph::new(Line::from(format!("{:.4}", self.area)))
            .block(Block::default().title("Area").borders(Borders::ALL));

        self.draw_chart(frame, chunks[1]);

        frame.render_widget(title, chunks[0]);
        frame.render_widget(area_footer, chunks[2]);

        if self.active_screen == CurrentScreen::Settings {
            self.draw_settings_popup(frame, self.popup_area(frame.area(), 80, 25));
        }
    }

    fn draw_settings_popup(&self, frame: &mut Frame, area_to_draw: Rect) {
        let settings_block = Block::bordered().title("Settings").bg(Color::Black);
        let vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area_to_draw);
        let top_horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ])
            .split(vertical_chunks[0]);
        let bottom_horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ])
            .split(vertical_chunks[1]);

        let function_block = Paragraph::new(Line::from(Span::styled(
            &self.function_text,
            match self.settings_focus {
                Settings::Function => Style::default().fg(Color::Red),
                _ => Style::default(),
            },
        )))
        .block(Block::default().title("Function").borders(Borders::ALL));

        let lower_bound_block = Paragraph::new(Line::from(Span::styled(
            &self.bounds_text[0],
            match self.settings_focus {
                Settings::LowerBound => Style::default().fg(Color::Red),
                _ => Style::default(),
            },
        )))
        .block(
            Block::default()
                .title("Lower Limit of Integration")
                .borders(Borders::ALL),
        );

        let upper_bound_block = Paragraph::new(Line::from(Span::styled(
            &self.bounds_text[1],
            match self.settings_focus {
                Settings::UpperBound => Style::default().fg(Color::Red),
                _ => Style::default(),
            },
        )))
        .block(
            Block::default()
                .title("Upper Limit of Integration")
                .borders(Borders::ALL),
        );

        let recalculate_area_block = Paragraph::new(Line::from(Span::styled(
            "Recalculate Area",
            match self.settings_focus {
                Settings::RecalculateArea => Style::default().fg(Color::Red),
                _ => Style::default(),
            },
        )))
        .block(Block::default().borders(Borders::ALL));

        let min_x = self.window_x_text[0].to_string();
        let max_x = self.window_x_text[1].to_string();
        let min_y = self.window_y_text[0].to_string();
        let max_y = self.window_y_text[1].to_string();

        let min_x_block = Paragraph::new(Line::from(Span::styled(
            &min_x,
            match self.settings_focus {
                Settings::MinimumX => Style::default().fg(Color::Red),
                _ => Style::default(),
            },
        )))
        .block(Block::default().title("Minimum X").borders(Borders::ALL));

        let max_x_block = Paragraph::new(Line::from(Span::styled(
            &max_x,
            match self.settings_focus {
                Settings::MaximumX => Style::default().fg(Color::Red),
                _ => Style::default(),
            },
        )))
        .block(Block::default().title("Maximum X").borders(Borders::ALL));

        let min_y_block = Paragraph::new(Line::from(Span::styled(
            &min_y,
            match self.settings_focus {
                Settings::MinimumY => Style::default().fg(Color::Red),
                _ => Style::default(),
            },
        )))
        .block(Block::default().title("Minimum Y").borders(Borders::ALL));

        let max_y_block = Paragraph::new(Line::from(Span::styled(
            &max_y,
            match self.settings_focus {
                Settings::MaximumY => Style::default().fg(Color::Red),
                _ => Style::default(),
            },
        )))
        .block(Block::default().title("Maximum Y").borders(Borders::ALL));

        frame.render_widget(Clear, area_to_draw);
        frame.render_widget(settings_block, area_to_draw);

        frame.render_widget(function_block, top_horizontal_chunks[0]);
        frame.render_widget(lower_bound_block, top_horizontal_chunks[1]);
        frame.render_widget(upper_bound_block, top_horizontal_chunks[2]);
        frame.render_widget(recalculate_area_block, top_horizontal_chunks[3]);

        frame.render_widget(min_x_block, bottom_horizontal_chunks[0]);
        frame.render_widget(max_x_block, bottom_horizontal_chunks[1]);
        frame.render_widget(min_y_block, bottom_horizontal_chunks[2]);
        frame.render_widget(max_y_block, bottom_horizontal_chunks[3]);
    }

    fn popup_area(&self, area: Rect, percent_x: u16, percent_y: u16) -> Rect {
        let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
        let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
        let [area] = vertical.areas(area);
        let [area] = horizontal.areas(area);
        area
    }

    fn draw_chart(&self, frame: &mut Frame, area_to_draw: Rect) {
        let x_labels = [
            Span::styled(
                format!("{}", self.window_x[0]),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}", (self.window_x[0] + self.window_x[1]) / 2.0),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}", self.window_x[1]),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ];

        let y_labels = [
            Span::styled(
                format!("{}", self.window_y[0]),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}", (self.window_y[0] + self.window_y[1]) / 2.0),
                Style::default(),
            ),
            Span::styled(
                format!("{}", self.window_y[1]),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ];

        let dataset = vec![
            Dataset::default()
                .name("Lower Bound Line")
                .marker(ratatui::symbols::Marker::Braille)
                .style(Style::default().fg(Color::Yellow))
                .data(&self.lower_bound_line),
            Dataset::default()
                .name("Upper Bound Line")
                .marker(ratatui::symbols::Marker::Braille)
                .style(Style::default().fg(Color::Yellow))
                .data(&self.upper_bound_line),
            Dataset::default()
                .name("X Axis Line")
                .marker(ratatui::symbols::Marker::Braille)
                .style(Style::default().fg(Color::Magenta))
                .data(&self.x_axis_line),
            Dataset::default()
                .name("Funtion")
                .marker(ratatui::symbols::Marker::Braille)
                .style(Style::default().fg(Color::Red))
                .data(&self.data),
        ];

        let chart = Chart::new(dataset)
            .block(Block::bordered())
            .x_axis(
                Axis::default()
                    .title("X Axis")
                    .style(Style::default().fg(Color::Gray))
                    .labels(x_labels)
                    .bounds(self.window_x),
            )
            .y_axis(
                Axis::default()
                    .title("Y Axis")
                    .style(Style::default().fg(Color::Gray))
                    .labels(y_labels)
                    .bounds(self.window_y),
            );

        frame.render_widget(chart, area_to_draw);
    }

    fn handle_events(&mut self) -> io::Result<()> {
        if let Event::Key(key_event) = event::read()? {
            match key_event.code {
                KeyCode::Char(character) => match self.settings_focus {
                    Settings::Function => self.function_text.push(character),
                    Settings::LowerBound => self.bounds_text[0].push(character),
                    Settings::UpperBound => self.bounds_text[1].push(character),
                    Settings::RecalculateArea => {}
                    Settings::MinimumX => self.window_x_text[0].push(character),
                    Settings::MaximumX => self.window_x_text[1].push(character),
                    Settings::MinimumY => self.window_y_text[0].push(character),
                    Settings::MaximumY => self.window_y_text[1].push(character),
                },
                KeyCode::Backspace => match self.settings_focus {
                    Settings::Function => _ = self.function_text.pop(),
                    Settings::LowerBound => _ = self.bounds_text[0].pop(),
                    Settings::UpperBound => _ = self.bounds_text[1].pop(),
                    Settings::RecalculateArea => {}
                    Settings::MinimumX => _ = self.window_x_text[0].pop(),
                    Settings::MaximumX => _ = self.window_x_text[1].pop(),
                    Settings::MinimumY => _ = self.window_y_text[0].pop(),
                    Settings::MaximumY => _ = self.window_y_text[1].pop(),
                },
                KeyCode::Enter => match self.settings_focus {
                    Settings::Function => {
                        self.expression = self.function_text.parse().unwrap();
                        self.populate_data();
                    }
                    Settings::LowerBound => {
                        self.bounds[0] = self.bounds_text[0].parse::<f64>().unwrap();
                        self.populate_data();
                    }
                    Settings::UpperBound => {
                        self.bounds[1] = self.bounds_text[1].parse::<f64>().unwrap();
                        self.populate_data();
                    }
                    Settings::RecalculateArea => self.calculate_area(),
                    Settings::MinimumX => {
                        self.window_x[0] = self.window_x_text[0].parse().unwrap();
                        self.populate_data();
                    }
                    Settings::MaximumX => {
                        self.window_x[1] = self.window_x_text[1].parse().unwrap();
                        self.populate_data();
                    }
                    Settings::MinimumY => {
                        self.window_y[0] = self.window_y_text[0].parse().unwrap();
                        self.populate_data();
                    }
                    Settings::MaximumY => {
                        self.window_y[1] = self.window_y_text[1].parse().unwrap();
                        self.populate_data();
                    }
                },
                KeyCode::Left => {
                    if self.settings_position_x != 0
                        && self.active_screen == CurrentScreen::Settings
                    {
                        self.settings_position_x -= 1;
                        self.settings_focus =
                            &SETTINGS_LAYOUT[self.settings_position_y][self.settings_position_x];
                    }
                }
                KeyCode::Right => {
                    if self.settings_position_x != 3
                        && self.active_screen == CurrentScreen::Settings
                    {
                        self.settings_position_x += 1;
                        self.settings_focus =
                            &SETTINGS_LAYOUT[self.settings_position_y][self.settings_position_x];
                    }
                }
                KeyCode::Up => {
                    if self.settings_position_y != 0
                        && self.active_screen == CurrentScreen::Settings
                    {
                        self.settings_position_y -= 1;
                        self.settings_focus =
                            &SETTINGS_LAYOUT[self.settings_position_y][self.settings_position_x];
                    }
                }
                KeyCode::Down => {
                    if self.settings_position_y != 1
                        && self.active_screen == CurrentScreen::Settings
                    {
                        self.settings_position_y += 1;
                        self.settings_focus =
                            &SETTINGS_LAYOUT[self.settings_position_y][self.settings_position_x];
                    }
                }
                KeyCode::Esc => match self.active_screen {
                    CurrentScreen::Main => self.exit(),
                    CurrentScreen::Settings => self.active_screen = CurrentScreen::Main,
                },
                KeyCode::Tab => {
                    self.active_screen = match self.active_screen {
                        CurrentScreen::Main => CurrentScreen::Settings,
                        CurrentScreen::Settings => CurrentScreen::Settings,
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}

fn main() -> io::Result<()> {
    let mut termial = ratatui::init();
    let app_result = App::new().run(&mut termial);
    ratatui::restore();
    app_result
}
