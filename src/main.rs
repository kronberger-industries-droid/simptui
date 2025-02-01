use core::*;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture, Event};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use prettytable::{row, Table};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Terminal;
use simptui::{detect_file_type, parse_markdown};
use std::fs;
use std::io;
use std::path::PathBuf;
use tui_textarea::{Input, Key, TextArea};
use walkdir::WalkDir;

#[derive(Debug)]
struct FileEntry {
    full_path: PathBuf,
    file_name: String,
}

struct App {
    textarea: TextArea<'static>,  // Input field
    is_valid: bool,               // Validity of the filename
    file_content: Option<String>, // Content of the file or error message
    scroll_offset: u16,           // Scroll position for file content
    should_redraw: bool,          // Redraw flag
    files: Vec<FileEntry>,        // List of files in the folder
    content_height: u16,          // Track content height for scrolling
}

impl App {
    fn new() -> Self {
        let mut textarea = TextArea::default();
        textarea.set_cursor_line_style(Style::default());
        textarea.set_placeholder_text("Enter a filename in this folder or any subfolder");

        let files = files_in_folder("./").unwrap_or_default();
        let is_valid = validate(&mut textarea, &files);

        Self {
            textarea,
            is_valid,
            file_content: None,
            scroll_offset: 0,
            should_redraw: true,
            files,
            content_height: 0,
        }
    }

    fn handle_input(&mut self, input: Input) -> bool {
        match input {
            Input { key: Key::Esc, .. } => true, // Exit on Esc
            Input {
                key: Key::Enter, ..
            } if self.is_valid => {
                let input = self.textarea.lines()[0].trim();
                if let Some(entry) = self.files.iter().find(|file| file.file_name == input) {
                    match fs::read_to_string(&entry.full_path) {
                        Ok(content) => {
                            match detect_file_type(&entry.full_path) {
                                "markdown" => {
                                    let equations = parse_markdown(&content);
                                    let mut table = Table::new();

                                    table.add_row(row!["Active", "Name", "Equation"]);

                                    for eq in &equations {
                                        table.add_row(row![
                                            if eq.active { "Yes" } else { "No" },
                                            eq.name,
                                            eq.body
                                        ]);
                                    }
                                    self.file_content = Some(table.to_string());
                                    self.scroll_offset = 0; // Reset scroll position
                                    self.content_height = self
                                        .file_content
                                        .as_ref()
                                        .map_or(0, |content| content.lines().count() as u16);
                                }
                                "csv" => {
                                    match Table::from_csv_file(&entry.full_path) {
                                        Ok(table) => {
                                            self.file_content = Some(table.to_string());
                                            self.scroll_offset = 0; // Reset scroll position
                                            self.content_height =
                                                self.file_content.as_ref().map_or(0, |content| {
                                                    content.lines().count() as u16
                                                });
                                        }
                                        Err(e) => {
                                            self.file_content =
                                                Some(format!("Error reading csv file: {} ", e))
                                        }
                                    }
                                }
                                "unknown" => {
                                    self.file_content = Some(content);
                                    self.scroll_offset = 0; // Reset scroll position
                                    self.content_height = self
                                        .file_content
                                        .as_ref()
                                        .map_or(0, |content| content.lines().count() as u16);
                                }
                                _ => {
                                    self.file_content =
                                        Some("Error detecting file type:".to_string())
                                }
                            }
                        }
                        Err(e) => self.file_content = Some(format!("Error reading file: {}", e)),
                    }
                } else {
                    self.file_content = Some("File not found!".to_string());
                }
                self.should_redraw = true;
                false
            }
            Input { key: Key::Up, .. } => {
                if self.scroll_offset > 0 {
                    self.scroll_offset -= 1;
                    self.should_redraw = true;
                }
                false
            }
            Input { key: Key::Down, .. } => {
                if self.scroll_offset < self.content_height.saturating_sub(1) {
                    self.scroll_offset += 1;
                    self.should_redraw = true;
                }
                false
            }
            Input {
                key: Key::PageUp, ..
            } => {
                self.scroll_offset = self.scroll_offset.saturating_sub(5); // Scroll up by 5 lines
                self.should_redraw = true;
                false
            }
            Input {
                key: Key::PageDown, ..
            } => {
                self.scroll_offset =
                    (self.scroll_offset + 5).min(self.content_height.saturating_sub(1)); // Scroll down by 5 lines
                self.should_redraw = true;
                false
            }
            input => {
                if self.textarea.input(input) {
                    self.is_valid = validate(&mut self.textarea, &self.files);
                    self.should_redraw = true;
                }
                false
            }
        }
    }

    fn draw(&mut self, term: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
        let size = term.size()?;
        let rect = Rect::new(0, 0, size.width, size.height);

        let layout = Layout::default()
            .constraints([
                Constraint::Length(3), // Input area
                Constraint::Min(1),    // File content area
            ])
            .split(rect);

        term.draw(|f| {
            // Input area
            f.render_widget(&self.textarea, layout[0]);

            // File content area
            let file_content = self
                .file_content
                .as_deref()
                .unwrap_or("No file content loaded.");
            let paragraph = Paragraph::new(file_content)
                .block(Block::default().borders(Borders::ALL).title("File Content"))
                .scroll((self.scroll_offset, 0)); // Apply vertical scroll offset
            f.render_widget(paragraph, layout[1]);
        })?;

        self.should_redraw = false;
        Ok(())
    }
}

fn files_in_folder(dir_path: &str) -> io::Result<Vec<FileEntry>> {
    let mut files = Vec::new();
    for entry in WalkDir::new(dir_path).into_iter().filter_map(|e| e.ok()) {
        if entry.path().is_file() {
            if let Some(file_name) = entry.file_name().to_str() {
                files.push(FileEntry {
                    full_path: entry.path().to_path_buf(),
                    file_name: file_name.to_string(),
                });
            }
        }
    }
    Ok(files)
}

fn validate(textarea: &mut TextArea, files: &[FileEntry]) -> bool {
    let input = textarea.lines()[0].trim();
    if files.iter().any(|file| file.file_name == input) {
        textarea.set_style(Style::default().fg(Color::LightGreen));
        textarea.set_block(
            Block::default()
                .border_style(Style::default().fg(Color::LightGreen))
                .borders(Borders::ALL)
                .title("OK"),
        );
        true
    } else {
        textarea.set_style(Style::default().fg(Color::LightRed));
        textarea.set_block(
            Block::default()
                .border_style(Style::default().fg(Color::LightRed))
                .borders(Borders::ALL)
                .title("ERROR: File not found"),
        );
        false
    }
}

fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<io::Stdout>>> {
    let mut stdout = io::stdout();
    enable_raw_mode()?;
    crossterm::execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

fn restore_terminal(term: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    crossterm::execute!(
        term.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    term.show_cursor()?;
    Ok(())
}

fn main() -> io::Result<()> {
    let mut term = setup_terminal()?;
    let mut app = App::new();

    loop {
        if app.should_redraw {
            app.draw(&mut term)?;
        }

        match crossterm::event::read()? {
            Event::Key(key) => {
                let input = Input::from(key);
                if app.handle_input(input) {
                    break;
                }
            }
            Event::Mouse(_) => {} // Ignore mouse events
            _ => {}
        }
    }

    restore_terminal(&mut term)?;
    println!("Input: {:?}", app.textarea.lines()[0]);
    Ok(())
}
