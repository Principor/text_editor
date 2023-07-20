use std::{io::{stdout, Write, Stdout}, time::Duration, env, fs};
use crossterm::{cursor, event::{self, Event, KeyEvent, KeyCode, KeyModifiers, KeyEventKind}, execute, queue, style, terminal::{self, ClearType}, Result};

mod text;
use text::{Text, Cursor, SearchData};

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
                self.search_data.find_results(phrase, &mut self.text)
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

    fn end_find(&mut self) {
        self.search_data.find_results(&String::from(""), &mut self.text);
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
        queue!(&mut self.w, cursor::MoveTo(0,0), terminal::Clear(ClearType::UntilNewLine), style::Print(welcome_message))?;
        queue!(&mut self.w, cursor::MoveTo(0,1), terminal::Clear(ClearType::UntilNewLine))
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
        self.print_header()?;
        for i in 2..self.win_size.1-1 {
            queue!(&mut self.w, cursor::MoveTo(0,i), style::Print("~"), terminal::Clear(ClearType::UntilNewLine))?;
            self.text_field.print_line(&mut self.w, (i-2) as usize)?;
        }
        let status_message = self.get_status_message();
        queue!(&mut self.w, cursor::MoveTo(0,self.win_size.1-1), terminal::Clear(ClearType::UntilNewLine), style::Print(status_message.as_str()))?;
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
        self.text_field.end_find();
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