use std::{io::{stdout, Write, Stdout}, time::Duration, env, fs, cmp::min};
use crossterm::{cursor, event::{self, Event, KeyEvent, KeyCode, KeyModifiers, KeyEventKind}, execute, queue, style::{self, Color, SetForegroundColor}, terminal::{self, ClearType}, Result};

macro_rules! prompt {
    ($editor:expr,$message:expr,$default:expr $(, $callback:expr)?) => {{
        let editor: &mut Editor = $editor;
        let message: &str = $message;
        let mut input: String = $default;
        loop {
            editor.set_status_message(Some(format!("{} {}", message, input)));
            editor.refresh_screen()?;
            let event = read_key()?;
            match event {
                KeyEvent{code: KeyCode::Esc, ..} => {
                    input.clear();
                    break;
                },
                KeyEvent{code: KeyCode::Enter, ..} => break,
                KeyEvent{code: KeyCode::Char(c), kind: KeyEventKind::Press, ..} => {
                    input.push(c);
                }
                KeyEvent{code: KeyCode::Backspace, kind: KeyEventKind::Press, ..} => {
                    if input.len() > 0 {input.remove(input.len() - 1);}
                }
                _ => {}
            }
            if let KeyEvent{code: _key_code, kind: KeyEventKind::Press, ..} = event {
                $($callback(editor, &input, _key_code);)?   
            }
        }
        if input.len() > 0 {
            Some(input)
        }else {
            None
        }
    }};
}

fn read_key() -> std::io::Result<KeyEvent> {
    loop {
        while event::poll(Duration::from_millis(500))? {
            if let Event::Key(event) = event::read()? {
                return Ok(event);
            }
        }
    }
}

#[derive(Clone)]
struct Cursor{
    x: u16,
    y: u16,
    render_x: u16,
    x_offset: u16,
    y_offset: u16,
    size: (u16, u16),
}

impl Cursor {
    fn new(size: (u16, u16)) -> Self {
        Self{x: 0, y: 0, render_x:0, x_offset: 0, y_offset: 0, size: size}
    }

    fn move_cursor(&mut self, text: &Text, direction: KeyCode) {
        match direction {
            KeyCode::Up => {
                if self.y > 0 {
                    self.y -= 1;
                    self.render_x = min(self.x, text.line_len(self.y as usize) as u16);
                }
            }
            KeyCode::Right => {
                self.x = self.render_x;
                if self.x < text.line_len(self.y as usize) as u16 {
                    self.x += 1;
                }else if self.y < text.len() as u16 - 1 {
                    self.y += 1;
                    self.x = 0;
                }
                self.render_x = self.x;
            }
            KeyCode::Down => {
                if self.y < text.len() as u16 - 1 {
                    self.y += 1;
                    self.render_x = min(self.x, text.line_len(self.y as usize) as u16)
                }
            }
            KeyCode::Left => {
                self.x = self.render_x;
                if self.x > 0 {
                    self.x -= 1;
                }else if self.y > 0 {
                    self.y -= 1;
                    self.x = text.line_len(self.y as usize) as u16;
                }
                self.render_x = self.x;
            }
            _ => {}
        }
    }

    fn change_offset(&mut self) {
        if self.y < self.y_offset{   // Up
            self.y_offset -= self.y_offset - self.y;
        }
        if self.render_x> self.size.0 + self.x_offset - 1{  // Right
            self.x_offset += self.render_x- (self.size.0 + self.x_offset - 1);
        }
        if self.y > self.size.1 + self.y_offset - 1{ // Down
            self.y_offset += self.y - (self.size.1 + self.y_offset - 1);
        }
        if self.render_x< self.x_offset{   // Left
            self.x_offset -= self.x_offset - self.render_x;
        }
    }

    fn get_position(&self) -> (u16, u16) {
        (self.render_x, self.y)
    }

    fn set_position(&mut self, x: u16, y: u16) {
        self.x = x;
        self.render_x = x;
        self.y = y;
    }

    fn get_offset(&self) -> (u16, u16) {
        (self.x_offset, self.y_offset)
    }

    fn get_line_index(&self) -> usize {
        self.y as usize
    }
}

struct SearchData {
    results: Vec<(u16,u16)>,
    index: usize,
}

impl SearchData {
    fn new() -> Self {
        Self{results: Vec::new(), index:0 }
    }

    fn find_results(&mut self, phrase: &String, text: &Text) -> Option<(u16, u16)> {
        self.results.clear();
        if phrase.len() == 0 { return None; }
        for row in 0..text.len() {
            let mut start = 0;
            while let Some(result) = text.find_phrase(phrase, row, start) {
                let col = start + result;
                self.results.push((col as u16, row as u16));
                start = col + phrase.len();
            }
        }
        if self.results.len() > 0 {
            Some(self.results[0])
        } else {
            None
        }
    }

    fn get_next(&mut self) -> Option<(u16, u16)> {
        if self.results.len() == 0 {
            None
        }else{
            self.index = (self.index + 1) % self.results.len();
            Some(self.results[self.index])
        }
    }

    fn get_previous(&mut self) -> Option<(u16, u16)> {
        if self.results.len() == 0 {
            None
        }else{
            self.index = (self.index + self.results.len() - 1) % self.results.len();
            Some(self.results[self.index])
        }
    }
}

enum HighlightType {
    Standard, 
    Number, 
}

trait SyntaxHighlight {
    fn update_syntax(&self, line: &mut Line);
    fn syntax_colour(&self, highlight_type: &HighlightType) -> Color;
}

struct RustSyntax {
}

impl SyntaxHighlight for RustSyntax {
    fn update_syntax(&self, line: &mut Line) {
        line.highlight_types = Vec::with_capacity(line.len());
        for c in line.content.chars() {
            if c.is_digit(10) {
                line.highlight_types.push(HighlightType::Number);
            }else{
                line.highlight_types.push(HighlightType::Standard);
            }
        }
    }

    fn syntax_colour(&self, highlight_type: &HighlightType) -> Color {
        match highlight_type {
            HighlightType::Number => Color::Cyan,
            _ => Color::Reset
        }
    }
}

struct Line {
    content: String,
    highlight_types: Vec<HighlightType>,
}

impl Line {
    fn new(content: String) -> Self {
        Self{content: content, highlight_types: Vec::new()}
    }

    fn blank() -> Self {
        Self{content: String::new(), highlight_types: Vec::new()}
    }

    fn insert(&mut self, index: usize, s: &str) {
        self.content.insert_str(index, s);
    }

    fn delete_char(&mut self, index: usize) {
        self.content.remove(index);
    }

    fn append(&mut self, line: &Line) {
        self.content.push_str(line.content.as_str());
    }

    fn split_at(&mut self, index: usize) -> Line {
        let new_line: String = self.content[index..].into();
        self.content = self.content[..index].into();
        Line::new(new_line)
    }

    fn find_phrase(&self, phrase: &str, start: usize) -> Option<usize> {
        self.content[start..].find(phrase)
    }

    fn len(&self) -> usize {
        self.content.len()
    }

    fn print(&self, w: &mut Stdout, start: usize, end: usize, highlight: &Option<Box<dyn SyntaxHighlight>>) -> std::io::Result<()> {
        let start = min(start as usize, self.len());
        let end = min(end as usize , self.len());
        for (i, c) in self.content[start..end].chars().enumerate() {
            let colour = match (highlight, self.highlight_types.get(i)) {
                (Some(syntax_highlight),Some(highlight_type)) => {
                    syntax_highlight.syntax_colour(highlight_type)
                }
                _ => {
                    Color::Reset
                }
            };
            queue!(w, SetForegroundColor(colour), style::Print(c))?;
        }
        Ok(())
    }
}

struct Text {
    lines: Vec<Line>,
    syntax_highlight: Option<Box<dyn SyntaxHighlight>>,
}

impl Text{
    fn new() -> Self {
        Self{lines: Vec::new(), syntax_highlight: Some(Box::new(RustSyntax{})) }
    }

    fn load(&mut self, content: std::io::Result<String>) {
        self.lines = match content {
            Ok(contents) => {
                let mut lines: Vec<Line> = contents.lines().map(|it| Line::new(it.into())).collect();
                if lines.len() == 0 {lines.push(Line::blank())}
                lines
            },
            _ => vec![Line::blank()]
        };
        for line in &mut self.lines {
            Text::update_line(&self.syntax_highlight, line);
        }
    }

    fn update_line(syntax_highlight: &Option<Box<dyn SyntaxHighlight>>, line: &mut Line) {
        if let Some(highlight) = syntax_highlight {
            highlight.update_syntax(line);
        }
    }

    fn save(&mut self, file_name: &String) -> std::io::Result<()> {
        let mut file = fs::OpenOptions::new().write(true).create(true).open(file_name)?;
        let strings: Vec<String> = self.lines.iter().map(|it| it.content.clone()).collect();
        let contents = strings.join("\n");
        file.set_len(contents.len() as u64)?;
        file.write_all(contents.as_bytes())
    }

    fn print_line(&self, w: &mut Stdout, index: usize, start: u16, end: u16) -> std::io::Result<()> {
        if index < self.lines.len() {
            let line = &self.lines[index];
            line.print(w, start as usize, end as usize, &self.syntax_highlight)?;
        }
        Ok(())
    }

    fn insert_char(&mut self, c: char, cursor: &mut Cursor) {
        let (x, y) = cursor.get_position();
        let line = &mut self.lines[cursor.get_line_index()];
        match c{
            '\t' => {
                line.insert(x as usize, "    ");
                cursor.set_position(x + 4, y)
            }
            _ => {
                line.insert(x as usize, c.to_string().as_str());
                cursor.set_position(x + 1, y)
            }
        }
        Text::update_line(&self.syntax_highlight, line);
    }

    fn new_line(&mut self, cursor: &mut Cursor) {
        let line_index = cursor.get_line_index();
        let (x,y) = cursor.get_position();
        let new_line = self.lines[line_index].split_at(x as usize);
        self.lines.insert(line_index + 1, new_line);
        Text::update_line(&self.syntax_highlight, &mut self.lines[line_index]);
        Text::update_line(&self.syntax_highlight, &mut self.lines[line_index + 1]);
        cursor.set_position(0, y + 1);
    }

    fn delete_char(&mut self, cursor: &mut Cursor) {
        let line_index = cursor.get_line_index();
        let (x, y) = cursor.get_position();
        if x > 0 {
            self.lines[line_index].delete_char(x as usize - 1);
            Text::update_line(&self.syntax_highlight, &mut self.lines[line_index]);
            cursor.set_position(x - 1, y);
        }else if y > 0 {
            let old_line = self.lines.remove(line_index);
            let old_length = self.lines[line_index-1].len();
            self.lines[line_index-1].append(&old_line);
            Text::update_line(&self.syntax_highlight, &mut self.lines[line_index-1]);
            cursor.set_position(old_length as u16, y-1);
        }
    }

    fn find_phrase(&self, phrase: &str, index: usize, start: usize) -> Option<usize> {
        self.lines[index].find_phrase(phrase, start)
    }

    fn len(&self) -> usize {
        self.lines.len()
    }

    fn line_len(&self, index: usize) -> usize {
        if index < self.len() {
            self.lines[index].len()
        } else {
            0
        }
    }
}

struct TextField {
    size: (u16, u16),
    text: Text,
    dirty: bool,
    cursor: Cursor,
    search_data: SearchData,
}

impl TextField {
    fn new(size: (u16, u16)) -> Self {
        Self{
            size: size, 
            text: Text::new(), 
            dirty: true, 
            cursor: Cursor::new(size.clone()), 
            search_data: SearchData::new(),
        }
    }

    fn load(&mut self, file_name: &String) {
        self.cursor.set_position(0, 0);
        self.dirty = false;
        let file_contents = fs::read_to_string(&file_name);
        self.text.load(file_contents);
    }

    fn save(&mut self, file_name: &String) -> std::io::Result<()>{
        self.text.save(file_name)?;
        self.dirty = false;
        Ok(())
    }

    fn print_line(&self, w: &mut Stdout, y: usize) -> std::io::Result<()> {
        let (x_offset, y_offset) = self.cursor.get_offset();
        let line_index = y + y_offset as usize;
        queue!(w, cursor::MoveTo(2, 2 + y as u16))?;
        self.text.print_line(w, line_index, x_offset, x_offset + self.size.0)?;
        Ok(())
    }

    fn get_cursor_position(&self) -> (u16, u16) {
        let (x, y) = self.cursor.get_position();
        let (x_offset, y_offset) = self.cursor.get_offset();
        (x + 2 - x_offset, y + 2 - y_offset)
    }

    fn move_cursor(&mut self, direction: KeyCode) {
        let cursor = &mut self.cursor;
        cursor.move_cursor(&self.text, direction);
        self.cursor.change_offset();
    }

    fn find_phrase(&mut self, phrase: &String, key_code: KeyCode) {
        let position = match key_code {
            KeyCode::Char(_) | KeyCode::Backspace => {
                self.search_data.find_results(phrase, &self.text)
            },
            KeyCode::Right => {
                self.search_data.get_next()
            },
            KeyCode::Left => {
                self.search_data.get_previous()
            }
            _ => None
        };
        if let Some((x, y)) = position {
            self.cursor.set_position(x, y);
            self.cursor.change_offset();
        }
    }

    fn insert_char(&mut self, c: char) {
        self.text.insert_char(c, &mut self.cursor);
        self.cursor.change_offset();
        self.dirty = true;
    }

    fn new_line(&mut self) {
        self.text.new_line(&mut self.cursor);
        self.cursor.change_offset();
        self.dirty = true;
    }

    fn delete_char(&mut self) {
        self.text.delete_char(&mut self.cursor);
        self.cursor.change_offset();
        self.dirty = true;
    }

    fn is_dirty(&self) -> bool {
        self.dirty
    }
}

struct Editor {
    running: bool,
    win_size: (u16, u16),
    w: Stdout,
    file_name: Option<String>,
    text_field: TextField,
    status_message: Option<String>,
    search_phrase: String,
}

impl Editor{
    fn new() -> Self {
        crossterm::terminal::enable_raw_mode().unwrap();
        let win_size = terminal::size().unwrap();
        let file_name = env::args().nth(1);
        let mut text_field = TextField::new((win_size.0 - 2, win_size.1 - 3));
        if let Some(name) = &file_name {
            text_field.load(name);
        }
        Self { running: true, win_size, w: stdout(), file_name: file_name, text_field: text_field, status_message: None, search_phrase: String::new()}
    }

    fn print_header(&mut self) -> std::io::Result<()> {
        let ver = option_env!("CARGO_PKG_VERSION").expect("Could not find version");
        let file_name = match &self.file_name {
            Some(name) => format!("{}{}", if self.text_field.is_dirty() {"*"} else {""}, name),
            None => String::from("Untitled")
        };
        let mut welcome_message = format!("{} -- Christopher's text editor -- {}", file_name, ver);
        welcome_message.truncate(self.win_size.0 as usize);
        queue!(&mut self.w, cursor::MoveTo(0,0), style::Print(welcome_message))
    }

    fn set_status_message(&mut self, message: Option<String>) {
        self.status_message = message;
    }

    fn get_status_message(&self) -> String {
        match &self.status_message {
            Some(string) => string.clone(),
            None => {
                let (x, y) = self.text_field.cursor.get_position();
                format!("Cursor: {}, {} -- {} lines", x, y, self.text_field.text.len())
            }
        }
    }

    fn refresh_screen(&mut self) -> std::io::Result<()> {
        queue!(&mut self.w, terminal::Clear(ClearType::All))?;
        self.print_header()?;
        for i in 2..self.win_size.1-1 {
            queue!(&mut self.w, cursor::MoveTo(0,i), style::Print("~"))?;
            self.text_field.print_line(&mut self.w, (i-2) as usize)?;
        }
        let status_message = self.get_status_message();
        queue!(&mut self.w, cursor::MoveTo(0,self.win_size.1-1), style::Print(status_message.as_str()))?;
        let cursor_position = self.text_field.get_cursor_position();
        queue!(&mut self.w, cursor::MoveTo(cursor_position.0, cursor_position.1), cursor::Show)?;
        self.w.flush()
    }

    fn save(&mut self) -> std::io::Result<()> {
        let default = if let Some(name) = &self.file_name {
            name.clone()
        }else{
            String::with_capacity(32)
        };
        self.file_name = prompt!(self, "Enter a path to save to:", default);
        match &self.file_name {
            Some(name) => self.text_field.save(name)?,
            _ => {},
        }
        Ok(())
    }

    fn load(&mut self) -> std::io::Result<()> {
        let default = if let Some(name) = &self.file_name {
            name.clone()
        }else{
            String::with_capacity(32)
        };
        self.file_name = prompt!(self, "Enter a path to load to:", default);
        match &self.file_name {
            Some(name) => {
                self.text_field.load(name)
            },
            _ => {},
        }
        Ok(())
    }

    fn find_phrase(editor: &mut Editor, input: &String, key_code: KeyCode) {
        editor.text_field.find_phrase(input, key_code);
    }

    fn find(&mut self) -> std::io::Result<()> {
        let previous_cursor = self.text_field.cursor.clone();
        let default_search = self.search_phrase.clone();
        let phrase = prompt!(self, "Find:", default_search, Editor::find_phrase);
        match phrase {
            Some(phrase) => self.search_phrase = phrase,
            None => self.text_field.cursor = previous_cursor,
        }
        Ok(())
    }

    fn quit(&mut self) -> std::io::Result<()>{
        self.set_status_message(Some(String::from("Press Ctrl-C again to confirm quit. Press Esc to cancel")));
        loop {
            self.refresh_screen()?;
            execute!(&mut self.w, cursor::Hide)?;
            if let KeyEvent {code: c, modifiers: m, kind: KeyEventKind::Press, ..} = read_key()? {
                match (c, m) {
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                        self.running = false;
                        break;
                    },
                    (KeyCode::Esc, _) => break,
                    _ => ()
                }
            }
        }
        Ok(())
    }

    fn run(&mut self) -> std::io::Result<()> {
        while self.running {
            self.set_status_message(None);
            self.refresh_screen()?;
            match read_key()? {
                KeyEvent{
                    code: KeyCode::Char('c'),
                    modifiers: event::KeyModifiers::CONTROL,
                    ..
                } => self.quit()?,
                KeyEvent{
                    code: KeyCode::Char('s'),
                    modifiers: event::KeyModifiers::CONTROL,
                    ..
                } => self.save()?,
                KeyEvent{
                    code: KeyCode::Char('l'),
                    modifiers: event::KeyModifiers::CONTROL,
                    ..
                } => self.load()?,
                KeyEvent{
                    code: KeyCode::Char('f'),
                    modifiers: event::KeyModifiers::CONTROL,
                    ..
                } => self.find()?,
                KeyEvent {
                    code: direction @ (KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right),
                    modifiers: event::KeyModifiers::NONE,
                    kind: KeyEventKind::Press,
                    ..
                } => self.text_field.move_cursor(direction),
                KeyEvent {
                    code: code @ (KeyCode::Char(..) | KeyCode::Tab),
                    kind: event::KeyEventKind::Press,
                    ..
                } => self.text_field.insert_char(match code {
                    KeyCode::Tab => '\t',
                    KeyCode::Char(ch) => ch,
                    _ => unreachable!(),
                }),
                KeyEvent {
                    code: KeyCode::Enter,
                    kind: event::KeyEventKind::Press,
                    ..
                } => self.text_field.new_line(),
                KeyEvent {
                    code: KeyCode::Backspace,
                    kind: event::KeyEventKind::Press,
                    ..
                } => self.text_field.delete_char(),
                _ => {}
            }
        }
        execute!(&mut self.w, terminal::Clear(ClearType::All), cursor::MoveTo(0, 0))
    }
}

fn main() -> Result<()> {
    let mut editor = Editor::new();
    editor.run()
}